#include <Arduino.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include <ESPmDNS.h>
#include <esp_wifi.h>

// ============================================================================
// FIELD CONFIGURATION
// ============================================================================
// Transparent UART <-> UDP bridge between Teensy and ground station.
// TODO: Confirm credentials, addresses, ports, UART settings, and framing.
// Changes to these values require rebuilding and uploading the firmware.

// Wi-Fi credentials and hostname
constexpr char WIFI_SSID[] = "MONOPROP_FIELD_ROUTER";
constexpr char WIFI_PASSWORD[] = "YOUR_ROUTER_PASSWORD";
constexpr char MDNS_HOSTNAME[] = "monoprop-bridge";

// ESP32 network configuration (true = DHCP, false = static IP)
// Match Monoprop/GroundControl's current ESP32 destination
constexpr bool USE_ESP_DHCP = false;
const IPAddress ESP_STATIC_IP(192, 168, 1, 50);
// verify gateway and subnet against the field router.
const IPAddress ROUTER_GATEWAY(192, 168, 1, 1);
const IPAddress ROUTER_SUBNET(255, 255, 255, 0);

// Ground station destination
// false = fixed IP, true = learn IP from incoming UDP packets
// use the ground laptop's actual address
constexpr bool USE_DYNAMIC_GROUND_IP = false;
const IPAddress GROUND_STATIC_IP(192, 168, 1, 100);

// UDP ports matching the current Monoprop/GroundControl defaults
constexpr uint16_t ESP_LISTEN_PORT = 8888;
constexpr uint16_t GROUND_TARGET_PORT = 9999;

// Teensy UART configuration (3.3V logic, crossed TX/RX, common GND)
// Baud matches Monoprop/Drone.
// Teensy TX pin 1 -> ESP RX GPIO16; ESP TX GPIO17 -> Teensy RX pin 0.
constexpr uint32_t UART_BAUD_RATE = 115200;
constexpr int8_t ESP_RX_PIN = 16;
constexpr int8_t ESP_TX_PIN = 17;

// Status LED
constexpr int STATUS_LED_PIN = 13;

// Buffer sizes and processing limits
constexpr size_t UART_RX_BUF_SZ = 4096;
constexpr size_t UART_TX_BUF_SZ = 2048;
constexpr size_t MAX_UDP_PAYLOAD = 1024;
constexpr size_t UART_WORK_PER_PASS = 256;

// Retry and diagnostic intervals (milliseconds)
constexpr uint32_t WIFI_RETRY_MS = 5000;
constexpr uint32_t SERVICE_RETRY_MS = 2000;
constexpr uint32_t DIAGNOSTIC_INTERVAL_MS = 5000;

// ============================================================================
// UART -> UDP PACKAGING - SELECT AFTER AGREEING THE ENDPOINT INTERFACE
// ============================================================================
// TODO: Agree framing, maximum frame size and recovery after missing bytes.
// This bridge does not define telemetry structures or decode command formats.
enum PackagingMode : uint8_t {
  MODE_IDLE_TIMEOUT, // Provisional byte batching after software-read inactivity.
  MODE_DELIMITER,    // Include the selected ending byte in each batch.
  MODE_FIXED_SIZE,   // Send after exactly FIXED_PACKET_SIZE bytes.
};

// Select the packaging mode here or override it with a build flag.
#ifndef BRIDGE_PACKAGING_MODE
#define BRIDGE_PACKAGING_MODE MODE_IDLE_TIMEOUT
#endif
constexpr PackagingMode PACKAGING_MODE = BRIDGE_PACKAGING_MODE;

// IDLE TIMEOUT:
// Send buffered bytes after a period without UART reads. Timing is
// based on software reads, so message boundaries aren't guaranteed.
constexpr uint32_t IDLE_GAP_US = 500;

// DELIMITER:
// Send when the specified ending byte is received.
constexpr uint8_t FRAME_DELIMITER = '\n';

// FIXED SIZE: replace 64 with the agreed serialized frame size
constexpr size_t FIXED_PACKET_SIZE = 64;

// Validate configuration at compile time
static_assert(
    PACKAGING_MODE == MODE_IDLE_TIMEOUT ||
    PACKAGING_MODE == MODE_DELIMITER ||
    PACKAGING_MODE == MODE_FIXED_SIZE,
    "Unsupported packaging mode"
);

static_assert(FIXED_PACKET_SIZE > 0 && FIXED_PACKET_SIZE <= MAX_UDP_PAYLOAD,
              "Invalid fixed packet size");

// Keep batches within the WiFiUDP library's send/receive buffer capacity.
static_assert(MAX_UDP_PAYLOAD > 0 && MAX_UDP_PAYLOAD <= 1460,
              "UDP payload size must be between 1 and 1460 bytes");

static_assert(UART_WORK_PER_PASS > 0 && UART_TX_BUF_SZ > 128,
              "Invalid UART configuration");

// ============================================================================
// RUNTIME STATES
// ============================================================================
WiFiUDP udp;
IPAddress targetGroundIP = GROUND_STATIC_IP;
bool hasGroundTarget = !USE_DYNAMIC_GROUND_IP;
bool udpReady = false;
bool mdnsReady = false;
bool linkReady = false;
bool staticConfigOK = true;
bool uartReady = false;
IPAddress boundIP;
// Downlink: a batch assembled from Teensy bytes, up to the UDP payload cap.
uint8_t teensyRxBuf[MAX_UDP_PAYLOAD];
size_t teensyRxLen = 0;
uint32_t lastByteMicros = 0;
// Uplink: one whole accepted datagram, forwarded incrementally. The offset
// tracks bytes already handed to the UART driver, not acknowledged by Teensy.
uint8_t groundRxBuf[MAX_UDP_PAYLOAD];
size_t groundRxLen = 0;
size_t groundTxOffset = 0;
uint32_t lastWiFiAttempt = 0;
uint32_t lastSocketAttempt = 0;
uint32_t lastMdnsAttempt = 0;
uint32_t lastDiagnostic = 0;
char diagnosticText[420];
size_t diagnosticLen = 0, diagnosticOffset = 0;

// Diagnostic counters (local activity and errors)
struct Diagnostics {
  uint32_t linkResets = 0;
  uint32_t retries = 0;
  uint32_t bindErrors = 0;
  uint32_t mdnsErrors = 0;
  uint32_t txPackets = 0;
  uint32_t txErrors = 0;
  uint32_t rxPackets = 0;
  uint32_t oversized = 0;
  uint32_t readErrors = 0;
  uint32_t discardedUART = 0;
  uint32_t discardedUplink = 0;
  uint32_t uartRxBytes = 0;
  uint32_t uartTxBytes = 0;
  uint32_t batchSplits = 0;
  uint32_t noTarget = 0;
  uint32_t configErrors = 0;
} stats;

// ============================================================================
// WI-FI CALLBACK - SIGNAL ONLY; DO ALL RECOVERY IN THE APPLICATION LOOP
// ============================================================================
// The event task only publishes state under this lock. Buffers, sockets, LED,
// logging and reconnect operations belong exclusively to the application loop.
portMUX_TYPE eventMux = portMUX_INITIALIZER_UNLOCKED;
uint32_t eventGeneration = 0;
uint32_t handledGeneration = 0;
void onWiFiEvent(WiFiEvent_t event) {
  if (event == ARDUINO_EVENT_WIFI_STA_DISCONNECTED ||
      event == ARDUINO_EVENT_WIFI_STA_LOST_IP ||
      event == ARDUINO_EVENT_WIFI_STA_GOT_IP) {
    portENTER_CRITICAL(&eventMux);
    ++eventGeneration;
    portEXIT_CRITICAL(&eventMux);
  }
}

// ============================================================================
// BUFFER CLEANUP AND NETWORK SERVICE LIFECYCLE
// ============================================================================
// Drop staged bytes instead of replaying old application buffers after an outage.
// Bytes already queued inside the UART driver cannot be recalled by this reset.
void discardBufferedData() {
  stats.discardedUART += teensyRxLen;
  teensyRxLen = 0;
  stats.discardedUplink += groundRxLen - groundTxOffset;
  groundRxLen = groundTxOffset = 0;
}

void drainUART() {
  // Bounded even when Teensy transmits continuously; runs during startup too.
  for (size_t n = 0; n < UART_RX_BUF_SZ && Serial1.available() > 0; ++n) {
    if (Serial1.read() >= 0) ++stats.discardedUART;
  }
}

void stopServices() {
  udp.stop(); // Also releases any partially read datagram.
  udpReady = false;
  if (mdnsReady) MDNS.end();
  mdnsReady = false;
  hasGroundTarget = !USE_DYNAMIC_GROUND_IP;
  targetGroundIP = GROUND_STATIC_IP;
  discardBufferedData();
}

void serviceNetwork(uint32_t now) {
  // Snapshot the callback's event counter under the same short lock. A plain
  // volatile variable would not replace synchronization across ESP32 tasks.
  uint32_t generation;
  portENTER_CRITICAL(&eventMux);
  generation = eventGeneration;
  portEXIT_CRITICAL(&eventMux);
  const IPAddress ip = WiFi.localIP();
  const bool connected = WiFi.status() == WL_CONNECTED && ip != IPAddress();
  // Generation detects even a disconnect/reconnect completed between two loops.
  if (generation != handledGeneration || connected != linkReady ||
      (connected && ip != boundIP)) {
    handledGeneration = generation;
    stopServices();
    drainUART();
    ++stats.linkResets;
    linkReady = connected;
    boundIP = ip;
    // Make both services eligible to start immediately on the new connection.
    // Unsigned elapsed-time subtraction also handles millis() wraparound.
    lastSocketAttempt = now - SERVICE_RETRY_MS;
    lastMdnsAttempt = now - SERVICE_RETRY_MS;
    Serial.printf("[WiFi] %s IP=%s\n", connected ? "connected" : "unavailable",
                  ip.toString().c_str());
  }
  // Solid LED = router association + IP; blinking = unavailable. This does not
  // prove UDP is bound, the ground program is listening, or Teensy is healthy.
  digitalWrite(STATUS_LED_PIN, connected ? HIGH : ((now / 250) % 2));
  if (!connected) {
    if (staticConfigOK && now - lastWiFiAttempt >= WIFI_RETRY_MS) {
      lastWiFiAttempt = now;
      ++stats.retries;
      WiFi.reconnect(); // One retry owner; no retry loop in the event callback.
    }
    return;
  }
  // Retry failed binding separately from Wi-Fi. Recreate sockets after IP/link
  // changes so a socket from the old connection cannot remain in use.
  if (!udpReady && now - lastSocketAttempt >= SERVICE_RETRY_MS) {
    lastSocketAttempt = now;
    udp.stop();
    udpReady = udp.begin(ESP_LISTEN_PORT) == 1;
    if (!udpReady) ++stats.bindErrors;
  }
  // mDNS failure does not stop UDP forwarding by numeric address.
  if (!mdnsReady && now - lastMdnsAttempt >= SERVICE_RETRY_MS) {
    lastMdnsAttempt = now;
    MDNS.end();
    mdnsReady = MDNS.begin(MDNS_HOSTNAME);
    if (!mdnsReady) ++stats.mdnsErrors;
  }
}

// ============================================================================
// DOWNLINK: TEENSY UART -> GROUND UDP
// ============================================================================
// Determine when the buffered UART data should be sent.
bool shouldFlushToUDP(uint8_t latestByte, bool newByteArrived) {
  if (!teensyRxLen) return false;
  if (teensyRxLen >= MAX_UDP_PAYLOAD) return true;

  if constexpr (PACKAGING_MODE == MODE_IDLE_TIMEOUT) {
    // Timeout is based on software reads, not physical UART timing.
    return Serial1.available() == 0 &&
           uint32_t(micros() - lastByteMicros) >= IDLE_GAP_US;

  } else if constexpr (PACKAGING_MODE == MODE_DELIMITER) {
    return newByteArrived && latestByte == FRAME_DELIMITER;

  } else if constexpr (PACKAGING_MODE == MODE_FIXED_SIZE) {
    return teensyRxLen >= FIXED_PACKET_SIZE;
  }

  return false;
}

// Send the buffered data to the ground station.
void flushDownlink() {
  if (!hasGroundTarget) {
    ++stats.noTarget;
  } else {
    const bool started =
        udp.beginPacket(targetGroundIP, GROUND_TARGET_PORT) == 1;

    const bool written =
        started && udp.write(teensyRxBuf, teensyRxLen) == teensyRxLen;

    if (written && udp.endPacket() == 1) {
      ++stats.txPackets;
    } else {
      ++stats.txErrors;
      stopServices();
      lastSocketAttempt = millis();
    }
  }

  teensyRxLen = 0;
}

// Read UART bytes and transmit completed batches.
void serviceDownlink() {
  for (size_t n = 0;
       n < UART_WORK_PER_PASS && Serial1.available() > 0;
       ++n) {

    const int b = Serial1.read();
    if (b < 0) break;

    teensyRxBuf[teensyRxLen++] = uint8_t(b);
    ++stats.uartRxBytes;
    lastByteMicros = micros();

    if (teensyRxLen == MAX_UDP_PAYLOAD &&
        PACKAGING_MODE == MODE_DELIMITER &&
        b != FRAME_DELIMITER) {
      ++stats.batchSplits;
    }

    if (shouldFlushToUDP(uint8_t(b), true)) {
      flushDownlink();
      return;
    }
  }

  if (shouldFlushToUDP(0, false)) {
    flushDownlink();
  }
}

// ============================================================================
// UPLINK: GROUND UDP -> TEENSY UART
// ============================================================================

// Forward UDP payloads to the Teensy without blocking UART transmission.
void serviceUplink() {
  // Receive a new packet only when the previous one is fully queued.
  if (groundRxLen == 0) {
    const int packetSize = udp.parsePacket();

    if (packetSize > 0) {
      ++stats.rxPackets;

      // Reject oversized packets completely.
      if (size_t(packetSize) > MAX_UDP_PAYLOAD) {
        ++stats.oversized;
        udp.flush();
        return;
      }

      const IPAddress sender = udp.remoteIP();
      const int len = udp.read(groundRxBuf, size_t(packetSize));
      udp.flush();

      if (len != packetSize) {
        ++stats.readErrors;
        return;
      }

      groundRxLen = size_t(len);
      groundTxOffset = 0;

      if constexpr (USE_DYNAMIC_GROUND_IP) {
        targetGroundIP = sender;
        hasGroundTarget = true;
      }
    }
  }

  // Forward pending bytes as UART buffer space becomes available.
  const int room = Serial1.availableForWrite();

  if (groundRxLen > groundTxOffset && room > 0) {
    size_t count = groundRxLen - groundTxOffset;

    if (count > size_t(room)) count = size_t(room);
    if (count > UART_WORK_PER_PASS) count = UART_WORK_PER_PASS;

    const size_t sent =
        Serial1.write(groundRxBuf + groundTxOffset, count);

    groundTxOffset += sent;
    stats.uartTxBytes += sent;

    if (groundTxOffset == groundRxLen) {
      groundRxLen = groundTxOffset = 0;
    }
  }
}

// ============================================================================
// PERIODIC DIAGNOSTICS
// ============================================================================

// Print bridge status and counters without waiting for Serial output.
void reportDiagnostics(uint32_t now) {
  if (diagnosticOffset == diagnosticLen &&
      now - lastDiagnostic >= DIAGNOSTIC_INTERVAL_MS) {

    lastDiagnostic = now;

    const int len = snprintf(
        diagnosticText, sizeof(diagnosticText),
        "[Bridge] link=%u udp=%u uart=%u resets=%lu retry=%lu "
        "bindErr=%lu mdnsErr=%lu configErr=%lu\n"
        "[Data] tx=%lu txErr=%lu rx=%lu oversize=%lu readErr=%lu "
        "uartRx=%lu uartTx=%lu dropUART=%lu dropUp=%lu "
        "noTarget=%lu splits=%lu\n",
        linkReady, udpReady, uartReady,
        (unsigned long)stats.linkResets,
        (unsigned long)stats.retries,
        (unsigned long)stats.bindErrors,
        (unsigned long)stats.mdnsErrors,
        (unsigned long)stats.configErrors,
        (unsigned long)stats.txPackets,
        (unsigned long)stats.txErrors,
        (unsigned long)stats.rxPackets,
        (unsigned long)stats.oversized,
        (unsigned long)stats.readErrors,
        (unsigned long)stats.uartRxBytes,
        (unsigned long)stats.uartTxBytes,
        (unsigned long)stats.discardedUART,
        (unsigned long)stats.discardedUplink,
        (unsigned long)stats.noTarget,
        (unsigned long)stats.batchSplits
    );

    diagnosticLen = len > 0 ? size_t(len) : 0;

    if (diagnosticLen >= sizeof(diagnosticText)) {
      diagnosticLen = sizeof(diagnosticText) - 1;
    }

    diagnosticOffset = 0;
  }

  // Queue as much output as the Serial buffer can accept.
  const int room = Serial.availableForWrite();

  if (room > 0 && diagnosticOffset < diagnosticLen) {
    size_t count = diagnosticLen - diagnosticOffset;

    if (count > size_t(room)) count = size_t(room);

    diagnosticOffset += Serial.write(
        reinterpret_cast<const uint8_t*>(diagnosticText) + diagnosticOffset,
        count
    );
  }
}

// ============================================================================
// STARTUP
// ============================================================================

void setup() {
  // Initialize status LED and debugging console.
  pinMode(STATUS_LED_PIN, OUTPUT);
  digitalWrite(STATUS_LED_PIN, LOW);
  Serial.begin(115200);

  // Configure UART buffers before starting communication.
  const size_t rxSize = Serial1.setRxBufferSize(UART_RX_BUF_SZ);
  const size_t txSize = Serial1.setTxBufferSize(UART_TX_BUF_SZ);

  Serial1.begin(UART_BAUD_RATE, SERIAL_8N1, ESP_RX_PIN, ESP_TX_PIN);

  uartReady = rxSize >= UART_RX_BUF_SZ &&
              txSize >= UART_TX_BUF_SZ &&
              bool(Serial1);

  if (!uartReady) ++stats.configErrors;

  // Configure Wi-Fi and connection management.
  WiFi.onEvent(onWiFiEvent);
  WiFi.mode(WIFI_STA);
  WiFi.setAutoReconnect(false);

  if constexpr (!USE_ESP_DHCP) {
    staticConfigOK = WiFi.config(
        ESP_STATIC_IP, ROUTER_GATEWAY, ROUTER_SUBNET
    );

    if (!staticConfigOK) ++stats.configErrors;
  }

  // Start Wi-Fi without waiting for a connection.
  if (staticConfigOK) WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  lastWiFiAttempt = millis();

  // Disable Wi-Fi power saving to reduce latency.
  if (esp_wifi_set_ps(WIFI_PS_NONE) != ESP_OK) {
    ++stats.configErrors;
  }

  Serial.println("[Bridge] Starting...");
}

// ============================================================================
// MAIN LOOP
// ============================================================================

void loop() {
  const uint32_t now = millis();
  serviceNetwork(now);

  if (linkReady && udpReady && uartReady) {
    serviceDownlink();

    // Downlink errors may close the UDP socket.
    if (udpReady) serviceUplink();
  } else {
    // Discard stale data while the bridge is unavailable.
    discardBufferedData();
    drainUART();
  }

  reportDiagnostics(now);
  delay(1);
}
