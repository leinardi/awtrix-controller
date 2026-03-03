# Awtrix3 MQTT Controller — Product Specification

> **Purpose:** This document specifies the complete behavior of an MQTT broker application that manages one or more [Awtrix Light / Awtrix3](https://blueforcer.github.io/awtrix-light/) LED matrix displays. It is intended as a language-agnostic blueprint for reimplementing this application in any language or runtime (originally Kotlin; target: Go). Implementation choices — concurrency model, file layout — are left to the implementer, with the exception of the MQTT broker library, which has an explicit recommendation in [§3.5](#35-recommended-go-library-comqtt).

---

## Table of Contents

1. [Functional Overview](#1-functional-overview)
2. [Configuration](#2-configuration)
3. [MQTT Broker](#3-mqtt-broker)
4. [MQTT Topic Contract](#4-mqtt-topic-contract)
5. [Data Models (JSON Schemas)](#5-data-models-json-schemas)
6. [Core Features & Behavior](#6-core-features--behavior)
   - 6.1 [Day / Night Mode](#61-day--night-mode)
   - 6.2 [Energy-Saving Mode](#62-energy-saving-mode)
   - 6.3 [Settings Push](#63-settings-push)
   - 6.4 [Birthday Notifications](#64-birthday-notifications)
   - 6.5 [New Year Notification](#65-new-year-notification)
   - 6.6 [Client State Tracking](#66-client-state-tracking)
   - 6.7 [Button Events](#67-button-events)
7. [Scheduler & Timed Events](#7-scheduler--timed-events)
8. [Error Handling Requirements](#8-error-handling-requirements)
9. [CLI Interface](#9-cli-interface)
10. [Deployment](#10-deployment)
11. [Security](#11-security)
12. [Behavioral Test Cases](#12-behavioral-test-cases)

---

## 1. Functional Overview

The application is an embedded MQTT broker (not an MQTT client) that Awtrix3 displays connect to directly. It provides:

| Feature | Description |
|---|---|
| **Embedded MQTT broker** | Accepts connections from any standard MQTT 3.1/3.1.1 client |
| **Day/Night theming** | Automatically adjusts display colors based on astronomical sunrise/sunset times at the configured location |
| **Energy-saving mode** | Reduces brightness to minimum during a configurable nightly window |
| **Birthday notifications** | Pushes a customizable notification to all displays at midnight on each configured birthday |
| **New Year notification** | Pushes a configurable "Happy New Year" notification to all displays at midnight on January 1st |
| **Client state tracking** | Maintains in-memory knowledge of each display's current app and latest stats |
| **Full Awtrix3 publish API** | Exposes all Awtrix3 MQTT output topics so other services can control displays by publishing through the broker |

The application has **no HTTP interface**. All communication with displays is via MQTT.

---

## 2. Configuration

### 2.1 File Location & Format

The application reads a single YAML configuration file. The path is configurable via a CLI flag or environment variable (see [§9](#9-cli-interface)), defaulting to `/etc/awtrix-controller/config.yaml`.

**Startup behavior:**

- If the file is **absent or unreadable**: log an error and exit with code `1`. There is no "start with defaults" for the file as a whole, because required fields (credentials, location) cannot have safe defaults.
- If the file is **present but invalid YAML**: log a parse error with line details and exit with code `1`.
- If the file is **present and valid**: required fields that are absent cause exit with code `1`; optional fields that are absent use the defaults documented in §2.3.

The configuration is loaded once at startup and is **not hot-reloaded**; a restart is required to pick up changes.

### 2.2 Full Schema

```yaml
# ─── MQTT Broker ──────────────────────────────────────────────
mqtt:
  port: 1883              # TCP port (default: 1883)
  ws_port:                # WebSocket port; omit to disable WebSocket support
  username: "awtrix"      # REQUIRED; clients must present this username
  password: "changeme"    # REQUIRED; clients must present this password

# ─── Location (used for sunrise/sunset calculation) ───────────
location:
  latitude: 48.137154     # REQUIRED. Decimal degrees, North positive
  longitude: 11.576124    # REQUIRED. Decimal degrees, East positive
  elevation: 530.0        # Metres above sea level (default: 0.0)

# ─── Timezone ─────────────────────────────────────────────────
# IANA timezone name. Controls all time-based features: energy-saving
# window, birthday alarms, New Year alarm, day/night scheduling.
# Strongly recommended to set explicitly. Defaults to the system timezone
# with a startup warning.
timezone: "Europe/Berlin"

# ─── Energy-Saving Window ────────────────────────────────────
energy_saving:
  start: "00:30"          # HH:MM in the configured timezone; window begins at this time
  end: "06:00"            # HH:MM in the configured timezone; window ends at this time
  # The window may span midnight (e.g. start=23:00, end=05:00 is valid).
  # During the window: brightness=1, auto-brightness=false.

# ─── Theme Colors ────────────────────────────────────────────
theme:
  day:
    calendar_accent: "#FF0000"   # Color for calendar header & active weekday
    content: "#FFFFFF"           # Color for text, date, inactive weekday
  night:
    calendar_accent: "#FF0000"
    content: "#474747"

# ─── Birthday List ───────────────────────────────────────────
birthdays:
  - date_of_birth: "1990-04-16"   # ISO-8601 (YYYY-MM-DD); year is used for age calculation
    name: "Alice"
    # All fields below are optional; defaults are shown.
    duration: 600                 # Notification display time in seconds (default: 600)
    icon: "14004"                 # LaMetric icon ID (default: 14004 — birthday cake)
    rainbow: true                 # Rainbow text effect (default: true)
    scroll_speed: 50              # Scroll speed in px/frame (default: 50)
    wakeup: true                  # Wake display from sleep (default: true)
    rtttl: "happybirthday:d=4,o=4,b=120:8d, 8d, e, d"   # RTTTL melody
    # message: auto-generated as "Happy <N> Birthday <name>!" where N is the
    # age at the time the alarm fires. Set to override.
    message: ""

# ─── New Year Notification ───────────────────────────────────
new_year:
  enabled: true           # Set to false to disable entirely (default: true)
  icon: "5855"            # LaMetric icon ID (default: 5855)
  duration: 600           # Display time in seconds (default: 600)
  rainbow: true           # Rainbow text effect (default: true)
  scroll_speed: 50        # Scroll speed in px/frame (default: 50)
  wakeup: true            # Wake display from sleep (default: true)
  # message: auto-generated as "Happy New Year <YYYY>!" where YYYY is the
  # incoming year, computed at fire time. Set to override.
  message: ""
```

### 2.3 Field Reference

#### `mqtt`

| Field | Type | Default | Description |
|---|---|---|---|
| `port` | integer | `1883` | MQTT TCP listen port |
| `ws_port` | integer? | — | WebSocket listen port; absent/null = disabled |
| `username` | string | — | **Required.** MQTT client username |
| `password` | string | — | **Required.** MQTT client password |

#### `location`

| Field | Type | Default | Description |
|---|---|---|---|
| `latitude` | float | — | **Required.** Decimal degrees (N positive) |
| `longitude` | float | — | **Required.** Decimal degrees (E positive) |
| `elevation` | float | `0.0` | Metres above sea level |

#### `timezone`

| Field | Type | Default | Description |
|---|---|---|---|
| `timezone` | string (IANA name) | System timezone | Timezone for all scheduled events and time comparisons. Example: `"Europe/Berlin"`, `"America/New_York"`, `"UTC"`. If absent, the system timezone is used and a startup warning is logged recommending explicit configuration. |

#### `energy_saving`

| Field | Type | Default | Description |
|---|---|---|---|
| `start` | string (`HH:MM`) | `"00:30"` | Window start time in the configured timezone |
| `end` | string (`HH:MM`) | `"06:00"` | Window end time in the configured timezone |

#### `theme.day` / `theme.night`

| Field | Type | Default (day / night) | Description |
|---|---|---|---|
| `calendar_accent` | `#RRGGBB` | `#FF0000` / `#FF0000` | Calendar header & active weekday color |
| `content` | `#RRGGBB` | `#FFFFFF` / `#474747` | Text, date, inactive weekday color |

#### Birthday entry

| Field | Type | Default | Description |
|---|---|---|---|
| `date_of_birth` | ISO-8601 date | **Required** | Year is used for age; month/day for yearly scheduling |
| `name` | string | **Required** | Displayed in the auto-generated message |
| `duration` | integer | `600` | Notification hold time (seconds) |
| `icon` | string | `"14004"` | LaMetric icon ID |
| `rainbow` | boolean | `true` | Rainbow text effect |
| `scroll_speed` | integer | `50` | Scroll speed (px/frame) |
| `wakeup` | boolean | `true` | Wake display from sleep |
| `rtttl` | string | *(Happy Birthday melody)* | RTTTL melody string |
| `message` | string | Auto-generated | Override message; if empty/absent, auto-generate `"Happy <N> Birthday <name>!"` |

#### `new_year`

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `true` | Set to `false` to disable the New Year notification entirely |
| `icon` | string | `"5855"` | LaMetric icon ID |
| `duration` | integer | `600` | Notification hold time (seconds) |
| `rainbow` | boolean | `true` | Rainbow text effect |
| `scroll_speed` | integer | `50` | Scroll speed (px/frame) |
| `wakeup` | boolean | `true` | Wake display from sleep |
| `message` | string | Auto-generated | Override message; if empty/absent, auto-generate `"Happy New Year <YYYY>!"` |

---

## 3. MQTT Broker

### 3.1 Protocol

- Implement or embed a standard MQTT **3.1 / 3.1.1** broker.
- Accept TCP connections on the configured port.
- Optionally accept WebSocket connections if `mqtt.ws_port` is configured.

### 3.2 Authentication

- Every connecting client **must** supply the configured `username` and `password`.
- Reject (disconnect) any client that supplies incorrect or absent credentials.
- All authenticated clients have equal access to all topics — no per-client ACLs.

### 3.3 Publish Behavior (outgoing)

All messages published by the application use:

- **QoS:** AT_LEAST_ONCE (QoS 1)
- **Retain:** `true` for `{clientId}/settings` publishes; `false` for all others

> **Why retain for settings:** Using `retain=true` on settings messages means the broker stores the last value and automatically delivers it to any client that subscribes to `{clientId}/settings`, regardless of when the publish occurred. This eliminates the race condition that would otherwise exist between the application detecting a client connection and that client completing its topic subscriptions. The device will always receive the current settings as soon as it subscribes, even if the application pushed settings before the device finished connecting.

### 3.4 Client Identity

The `clientId` that a device uses when connecting is the unique identifier used to address all per-device topics (see Section 4). The application must be able to enumerate all currently connected client IDs at any time.

### 3.5 Recommended Go Library: comqtt

[comqtt](https://github.com/wind-c/comqtt) (`github.com/wind-c/comqtt/v2`) is the recommended embedded MQTT broker library for the Go implementation. It is a strong fit for this project for the following reasons:

| Criterion | comqtt |
|---|---|
| **License** | MIT — no restrictions on embedding or distribution |
| **MQTT versions** | 3.0, 3.1.1, and 5.0 |
| **Embedding** | First-class: instantiate `mqtt.Server` directly in-process; no separate process needed |
| **TCP listener** | `listeners.NewTCP("t1", ":1883", nil)` |
| **WebSocket listener** | `listeners.NewWebsocket("ws1", ":8080", nil)` — enabled only when `mqtt.ws_port` is configured |
| **Authentication** | Hooks-based; implement the `OnConnectAuthenticate` hook to check credentials against the configured username/password |
| **Packet interception** | `OnPublish` hook intercepts all incoming device messages; `OnConnect` hook detects new connections |
| **Publish from server** | `server.Publish(topic, payload, retain, qos)` |
| **Connected clients** | `server.Clients.GetAll()` returns all active client sessions |
| **Default posture** | DENY-ALL by default — explicit auth hook required, which aligns with our security model |
| **Maintenance** | Actively maintained; MIT-licensed fork of the widely used `mochi-co/mqtt` |

**Minimal setup pattern:**

```go
import (
    "github.com/wind-c/comqtt/v2/mqtt"
    "github.com/wind-c/comqtt/v2/mqtt/hooks/auth"
    "github.com/wind-c/comqtt/v2/mqtt/listeners"
)

server := mqtt.New(nil)

// Replace auth.AllowHook with a custom hook that checks configured credentials.
// auth.AllowHook is shown here for illustration only — do NOT use in production.
_ = server.AddHook(new(auth.AllowHook), nil)

tcp := listeners.NewTCP("tcp1", ":1883", nil)
_ = server.AddListener(tcp)

// Optional WebSocket listener (only if ws_port is configured):
ws := listeners.NewWebsocket("ws1", ":8080", nil)
_ = server.AddListener(ws)

go server.Serve()
```

**Authentication hook pattern** (replace the AllowHook with this):

```go
type CredentialHook struct {
    mqtt.HookBase
    username string
    password string
}

func (h *CredentialHook) OnConnectAuthenticate(cl *mqtt.Client, pk packets.Packet) bool {
    return string(pk.Connect.Username) == h.username &&
           string(pk.Connect.Password) == h.password
}
```

**Relevant hooks for this application:**

- `OnConnectAuthenticate` → validate credentials
- `OnConnect` → detect new device connections; trigger initial settings push (see §6.3)
- `OnDisconnect` → detect device disconnections; clear in-memory client state (see §6.6)
- `OnPublish` → inspect `pk.TopicName` to handle `stats`, `currentApp`, and button topics

---

## 4. MQTT Topic Contract

`{clientId}` refers to the MQTT client ID of a connected Awtrix3 device.

### 4.1 Incoming Topics (Device → Application)

The application intercepts all publish packets on the broker via the `OnPublish` hook.

| Topic | Payload | Action |
|---|---|---|
| `{clientId}/stats` | JSON [`Stats`](#stats) | Update stored client stats |
| `{clientId}/stat/currentApp` | Plain string (app name) | Update stored current app for this client |
| `{clientId}/button/left` | `"0"` or `"1"` | Record button event (0 = released, 1 = pressed) |
| `{clientId}/button/select` | `"0"` or `"1"` | Record button event |
| `{clientId}/button/right` | `"0"` or `"1"` | Record button event |

**Connection-triggered settings push:** When a device connects (detected via the `OnConnect` hook), the application publishes the current settings to `{clientId}/settings` with `retain=true`. Because the message is retained, the device will receive it as soon as it subscribes to that topic, regardless of the order or timing of events after the CONNECT handshake. No subscribe-interception is needed.

### 4.2 Outgoing Topics (Application → Device)

The application may publish to any of these topics at any time. Payload is JSON unless marked *(empty)*.

| Topic | Payload type | Retain | Description |
|---|---|---|---|
| `{clientId}/notify` | [`Notification`](#notification) | false | Display a transient notification |
| `{clientId}/notify/dismiss` | *(empty)* | false | Dismiss the active notification |
| `{clientId}/settings` | [`Settings`](#settings) | **true** | Push device settings (retained) |
| `{clientId}/custom/{appName}` | [`CustomApp`](#customapp) | false | Create or update a custom app |
| `{clientId}/switch` | *(empty)* | false | Switch to the next app |
| `{clientId}/previousapp` | *(empty)* | false | Navigate to the previous app |
| `{clientId}/nextapp` | *(empty)* | false | Navigate to the next app |
| `{clientId}/doupdate` | *(empty)* | false | Trigger OTA firmware update |
| `{clientId}/apps` | *(empty)* | false | Request the app list from device |
| `{clientId}/power` | [`Power`](#power) | false | Turn display on/off |
| `{clientId}/sleep` | [`Sleep`](#sleep) | false | Put display to sleep |
| `{clientId}/indicator1` | [`Indicator`](#indicator) or *(empty)* | false | Set LED indicator 1; empty payload clears it |
| `{clientId}/indicator2` | [`Indicator`](#indicator) or *(empty)* | false | Set LED indicator 2 |
| `{clientId}/indicator3` | [`Indicator`](#indicator) or *(empty)* | false | Set LED indicator 3 |
| `{clientId}/reboot` | *(empty)* | false | Reboot the device |
| `{clientId}/moodlight` | [`MoodLight`](#moodlight) or *(empty)* | false | Set mood light; empty payload clears it |
| `{clientId}/sound` | *(empty)* | false | Play the default sound |
| `{clientId}/rtttl` | [`Rtttl`](#rtttl) | false | Play an RTTTL melody |
| `{clientId}/sendscreen` | *(empty)* | false | Request a screenshot from the device |
| `{clientId}/r2d2` | *(empty)* | false | Play an R2-D2 sound effect |

---

## 5. Data Models (JSON Schemas)

All JSON field names below are exactly as the Awtrix3 firmware expects them.

### `Stats`

Received on `{clientId}/stats`. All fields are required in the payload from the device.

```json
{
  "app":        "time",
  "bat":        95,
  "bat_raw":    3900,
  "bri":        128,
  "hum":        45,
  "indicator1": false,
  "indicator2": false,
  "indicator3": false,
  "ip_address": "192.168.1.50",
  "ldr_raw":    512,
  "lux":        300,
  "matrix":     true,
  "messages":   0,
  "ram":        120000,
  "temp":       22,
  "type":       1,
  "uid":        "aabbccddeeff",
  "uptime":     3600,
  "version":    "0.98",
  "wifi_signal": -55
}
```

| Field | Type | Description |
|---|---|---|
| `app` | string | Currently displayed app name |
| `bat` | integer | Battery level (0–100 %) |
| `bat_raw` | integer | Raw ADC battery value |
| `bri` | integer | Current brightness |
| `hum` | integer | Humidity (%) |
| `indicator1/2/3` | boolean | State of the three side LEDs |
| `ip_address` | string | Device IP address |
| `ldr_raw` | integer | Raw light-sensor ADC reading |
| `lux` | integer | Ambient light (lux) |
| `matrix` | boolean | Whether the matrix is enabled |
| `messages` | integer | Pending message count |
| `ram` | integer | Free RAM (bytes) |
| `temp` | integer | Temperature (°C) |
| `type` | integer | Hardware type identifier |
| `uid` | string | Device MAC / unique ID |
| `uptime` | integer | Uptime (seconds) |
| `version` | string | Firmware version string |
| `wifi_signal` | integer | WiFi RSSI (dBm) |

---

### `Settings`

Sent to `{clientId}/settings` with `retain=true`. All fields are **optional**; omit any field to leave the device's current setting unchanged.

```json
{
  "ATIME":      7000,
  "TEFF":       1,
  "TSPEED":     400,
  "TCOL":       "#FF0000",
  "TMODE":      0,
  "CHCOL":      "#FF0000",
  "CBCOL":      "#FFFFFF",
  "CTCOL":      "#FFFFFF",
  "WD":         true,
  "WDCA":       "#FF0000",
  "WDCI":       "#474747",
  "BRI":        128,
  "ABRI":       true,
  "ATRANS":     true,
  "CCORRECTION":[255, 255, 255],
  "CTEMP":      [255, 255, 255],
  "TFORMAT":    "%H:%M",
  "DFORMAT":    "%d.%m.%y",
  "SOM":        true,
  "BLOCKN":     false,
  "UPPERCASE":  false,
  "TIME_COL":   "#FFFFFF",
  "DATE_COL":   "#FFFFFF",
  "TEMP_COL":   "#FFFFFF",
  "HUM_COL":    "#FFFFFF",
  "BAT_COL":    "#FFFFFF",
  "SSPEED":     100,
  "TIM":        true,
  "DAT":        true,
  "HUM":        true,
  "TEMP":       true,
  "BAT":        true,
  "MATP":       false,
  "VOL":        15,
  "OVERLAY":    "clear"
}
```

#### `TEFF` — Transition effect

| Value | Effect |
|---|---|
| `0` | Random |
| `1` | Slide |
| `2` | Dim |
| `3` | Zoom |
| `4` | Rotate |
| `5` | Pixelate |
| `6` | Curtain |
| `7` | Ripple |
| `8` | Blink |
| `9` | Reload |
| `10` | Fade |

#### `TMODE` — Clock display mode

Values `0`–`4` select different clock face styles defined by the firmware.

#### `TFORMAT` — Time format strings

| Value | Example output |
|---|---|
| `%H:%M:%S` | `13:30:45` |
| `%l:%M:%S` | `1:30:45` |
| `%H:%M` | `13:30` |
| `%H %M` | `13.30` (blinking colon) |
| `%l:%M` | `1:30` |
| `%l %M` | `1:30` (blinking colon) |
| `%l:%M %p` | `1:30 PM` |
| `%l %M %p` | `1:30 PM` (blinking colon) |

#### `DFORMAT` — Date format strings

| Value | Example output |
|---|---|
| `%d.%m.%y` | `16.04.22` |
| `%d.%m` | `16.04` |
| `%y-%m-%d` | `22-04-16` |
| `%m-%d` | `04-16` |
| `%m/%d/%y` | `04/16/22` |
| `%m/%d` | `04/16` |
| `%d/%m/%y` | `16/04/22` |
| `%d/%m` | `16/04` |
| `%m-%d-%y` | `04-16-22` |

#### `OVERLAY` — Global weather overlay

Valid string values: `clear`, `snow`, `rain`, `drizzle`, `storm`, `thunder`, `frost`

---

### `Notification`

Sent to `{clientId}/notify`. All fields optional.

```json
{
  "text":       "Hello World",
  "textCase":   0,
  "topText":    false,
  "textOffset": 0,
  "center":     true,
  "color":      "#FFFFFF",
  "gradient":   ["#FF0000", "#0000FF"],
  "blinkText":  0,
  "fadeText":   0,
  "background": "#000000",
  "rainbow":    false,
  "icon":       "14004",
  "pushIcon":   0,
  "repeat":     1,
  "duration":   5,
  "hold":       false,
  "sound":      "chime",
  "rtttl":      "happybirthday:d=4,o=4,b=120:8d, 8d, e, d",
  "loopSound":  false,
  "bar":        [1, 2, 3, 4, 5],
  "line":       [1, 2, 3, 4, 5],
  "autoscale":  true,
  "progress":   50,
  "progressC":  "#00FF00",
  "progressBC": "#FF0000",
  "draw":       [{"dp": [0, 0, "#FF0000"]}],
  "stack":      true,
  "wakeup":     true,
  "noScroll":   false,
  "clients":    ["mydevice"],
  "scrollSpeed":100,
  "effect":     "Fade",
  "effectSettings": {"speed": 3, "palette": "Rainbow", "blend": true},
  "overlay":    "snow"
}
```

| Field | Type | Description |
|---|---|---|
| `text` | string | Notification text |
| `textCase` | integer | 0 = original, 1 = uppercase, 2 = lowercase |
| `topText` | boolean | Display text at top of matrix |
| `textOffset` | integer | Horizontal text offset (pixels) |
| `center` | boolean | Center text horizontally |
| `color` | `#RRGGBB` | Text color |
| `gradient` | array of `#RRGGBB` | Gradient colors for text |
| `blinkText` | integer | Blink rate (ms) |
| `fadeText` | integer | Fade rate (ms) |
| `background` | `#RRGGBB` | Background fill color |
| `rainbow` | boolean | Rainbow color cycling on text |
| `icon` | string | LaMetric icon ID |
| `pushIcon` | integer | Icon behavior: 0=static, 1=scroll, 2=fixed |
| `repeat` | integer | Repeat count; -1 = infinite |
| `duration` | integer | Display time (seconds) |
| `hold` | boolean | Hold until explicitly dismissed |
| `sound` | string | Sound file name (device filesystem) |
| `rtttl` | string | RTTTL melody string |
| `loopSound` | boolean | Loop the sound |
| `bar` | array of integer | Bar chart data values |
| `line` | array of integer | Line chart data values |
| `autoscale` | boolean | Auto-scale chart data to matrix height |
| `progress` | integer | Progress bar value (0–100) |
| `progressC` | `#RRGGBB` | Progress bar fill color |
| `progressBC` | `#RRGGBB` | Progress bar background color |
| `draw` | array of [Draw](#draw-commands) | Custom pixel drawing commands |
| `stack` | boolean | Queue alongside other notifications |
| `wakeup` | boolean | Wake display from sleep before showing |
| `noScroll` | boolean | Disable text scrolling |
| `clients` | array of string | Target specific client IDs; empty = all |
| `scrollSpeed` | integer | Scroll speed (pixels/frame) |
| `effect` | string | Background [effect](#effects) name |
| `effectSettings` | object | Effect configuration (see [EffectSettings](#effectsettings)) |
| `overlay` | string | Weather overlay (see [Overlay values](#overlay--global-weather-overlay)) |

> **Note on `clients` for internally generated notifications:** When the application generates birthday or New Year notifications, it fans out manually by iterating all connected client IDs and sending one `Notify` publish per device. The `clients` field is left empty in each published payload — it is not used to drive the fan-out; it is a device-side filtering field the firmware uses when it receives a notification forwarded from another source.

---

### `CustomApp`

Sent to `{clientId}/custom/{appName}`. Shares all fields with `Notification` (see above) plus:

| Additional field | Type | Description |
|---|---|---|
| `pos` | integer | Position in the app rotation order |
| `lifetime` | integer | App lifetime in seconds; `0` = infinite |
| `lifetimeMode` | integer | When to reset lifetime: `0` = on message, `1` = on view |
| `save` | boolean | Persist this app across device reboots |

Fields present in `Notification` but **not applicable** to `CustomApp`: `hold`, `stack`, `wakeup`.

---

### `Indicator`

Sent to `{clientId}/indicator1`, `indicator2`, or `indicator3`. Send an empty payload to clear the indicator.

```json
{
  "color": "#FF0000",
  "blink": 500,
  "fade":  0
}
```

| Field | Type | Description |
|---|---|---|
| `color` | string | Color in any format the device accepts (e.g. `"#FF0000"` or `"255 0 0"`) |
| `blink` | integer? | Blink interval in ms; omit or `0` for solid |
| `fade` | integer? | Fade speed; omit or `0` for no fade |

---

### `MoodLight`

Sent to `{clientId}/moodlight`. Send an empty payload to disable mood light.

```json
{
  "brightness": 128,
  "color": "#8800FF"
}
```

---

### `Power`

Sent to `{clientId}/power`.

```json
{ "power": true }
```

`true` = on, `false` = off.

---

### `Sleep`

Sent to `{clientId}/sleep`.

```json
{ "sleep": 30 }
```

Value is the number of seconds to sleep before auto-waking; `0` = sleep indefinitely.

---

### `Rtttl`

Sent to `{clientId}/rtttl`.

```json
{ "rtttl": "happybirthday:d=4,o=4,b=120:8d, 8d, e, d" }
```

---

### Draw Commands

Used in `Notification.draw` and `CustomApp.draw`. Each draw command is a JSON object with a single key naming the command and an array of arguments as the value.

| Command key | Arguments | Description |
|---|---|---|
| `dp` | `[x, y, "#color"]` | Set a single pixel |
| `dl` | `[x0, y0, x1, y1, "#color"]` | Draw a line |
| `dr` | `[x, y, w, h, "#color"]` | Draw a rectangle (outline) |
| `df` | `[x, y, w, h, "#color"]` | Draw a filled rectangle |
| `dc` | `[x, y, r, "#color"]` | Draw a circle (outline) |
| `dfc` | `[x, y, r, "#color"]` | Draw a filled circle |
| `dt` | `[x, y, "text", "#color"]` | Draw text at position |

Example: `[{"dp": [3, 4, "#FF0000"]}, {"dl": [0, 0, 7, 0, "#00FF00"]}]`

---

### Effects

Valid values for `Notification.effect` / `CustomApp.effect`:

`Fade`, `MovingLine`, `BrickBreaker`, `PingPong`, `Radar`, `Checkerboard`, `Fireworks`, `PlasmaCloud`, `Ripple`, `Snake`, `Pacifica`, `TheaterChase`, `Plasma`, `Matrix`, `SwirlIn`, `SwirlOut`, `LookingEyes`, `TwinklingStars`, `ColorWaves`

---

### EffectSettings

```json
{ "speed": 3, "palette": "Rainbow", "blend": true }
```

| Field | Type | Description |
|---|---|---|
| `speed` | integer | Animation speed |
| `palette` | string | Named color palette |
| `blend` | boolean | Blend between palette colors |

---

## 6. Core Features & Behavior

### 6.1 Day / Night Mode

**Purpose:** Adapt display colors to ambient conditions based on the time of day at the configured location.

**Trigger events:**

- At sunrise time → switch to **Day** mode
- At sunset time → switch to **Night** mode

**Determination at startup:** At application startup, calculate whether it is currently day or night in the configured timezone, based on whether the current time falls between today's sunrise and sunset. Apply the correct initial mode before the broker starts accepting connections.

**Mode switching:**

1. Compute the next sunrise and sunset times for the configured location using an astronomical algorithm (visual twilight — upper limb of the sun at the horizon is recommended).
2. Schedule a one-shot timer for whichever event comes next.
3. When that timer fires, switch mode and reschedule the next event for the following day.
4. On a mode change, push updated settings (see [§6.3](#63-settings-push)) to **all currently connected clients**.

**Polar latitude fallback:** At some latitudes there are periods where the sun never rises (polar night) or never sets (midnight sun), and astronomical libraries return no result for one or both events. In these cases:

- If sunrise cannot be computed (polar night): stay in **Night** mode for the current day; schedule a retry at the start of the next calendar day.
- If sunset cannot be computed (midnight sun): stay in **Day** mode for the current day; schedule a retry at the start of the next calendar day.
- Log a warning indicating the condition (not an error). Do not enter a retry loop.

**Color values applied on a Settings push** (sourced from `theme.day` / `theme.night` config):

| Settings field | Source |
|---|---|
| `CHCOL` (calendar header color) | `calendar_accent` |
| `CBCOL` (calendar background) | `content` |
| `WDCA` (weekday active color) | `calendar_accent` |
| `WDCI` (weekday inactive color) | `content` |
| `TIME_COL` (time display color) | `content` |
| `DATE_COL` (date display color) | `content` |

---

### 6.2 Energy-Saving Mode

**Purpose:** Reduce display brightness automatically during late-night hours to conserve power and avoid light pollution.

**Window:** Defined by `energy_saving.start` and `energy_saving.end` in the configured timezone. The window may span midnight.

**Behavior when energy-saving is ACTIVE:**

- `BRI` (brightness) = `1`
- `ABRI` (auto-brightness) = `false`

**Behavior when energy-saving is INACTIVE:**

- `BRI` is **not included** in the settings push. The Awtrix firmware treats `ABRI=true` as taking precedence over any previously set manual brightness value, so omitting `BRI` is intentional and correct.
- `ABRI` = `true`

**Trigger events:**

- At `start` time each day → activate energy-saving mode → push updated settings to all connected clients.
- At `end` time each day → deactivate energy-saving mode → push updated settings to all connected clients.

**Determination at startup:** Check whether the current time in the configured timezone falls within the configured window and apply the correct initial energy profile before the broker starts.

**Interaction with notifications:** Energy-saving mode affects only the persistent `Settings` push (brightness). Notifications (birthday, New Year, or any other) are **always sent regardless of the current energy-saving state**. The `wakeup: true` field on a notification wakes the display from sleep; however, the display will remain at the energy-saving brightness level (`BRI=1`) while energy saving is active. This is intentional — the application does not temporarily override brightness for notifications. Users who find notifications too dim during the energy-saving window should configure a later `start` time.

---

### 6.3 Settings Push

A settings push sends a `Settings` JSON object to `{clientId}/settings` with `retain=true`, containing only the fields managed by the application. The application does **not** send a full settings object — only the fields it actively controls. All other device settings (app times, transition effects, etc.) remain untouched.

**Fields the application always manages:**

| Settings field | Managed by |
|---|---|
| `CHCOL`, `CBCOL`, `WDCA`, `WDCI`, `TIME_COL`, `DATE_COL` | Day/Night theme |
| `BRI`, `ABRI` | Energy-saving profile (`BRI` only included when energy saving is active) |

**When settings are pushed:**

1. A new client connects (detected via the broker's `OnConnect` event)
2. Day/Night mode changes
3. Energy-saving mode activates or deactivates

In all cases, the two layers (theme + energy profile) are evaluated together and sent as a single settings message. Using `retain=true` ensures the device receives the current settings even if it subscribes to the topic after the push was sent.

---

### 6.4 Birthday Notifications

**Purpose:** Display a birthday message on all connected displays at midnight on each person's birthday.

**Scheduling:** For each entry in `birthdays`, schedule a repeating yearly alarm that fires at `00:00:00` in the configured timezone on the configured month and day.

**At alarm fire time:**

1. Compute the person's current age: `current_year - birth_year`. Since the alarm fires at midnight at the start of the birthday, the person is turning that age today, so no further adjustment is needed.
2. Build the notification text:
   - If `message` is empty or absent: generate `"Happy <N> Birthday <name>!"` where `<N>` is the computed age.
   - If `message` is explicitly set: use it verbatim.
3. Build a `Notification` with:
   - `text` = computed or configured message
   - `icon` = configured `icon`
   - `duration` = configured `duration`
   - `rainbow` = configured `rainbow`
   - `rtttl` = configured `rtttl`
   - `loopSound` = `false` (always)
   - `scrollSpeed` = configured `scroll_speed`
   - `wakeup` = configured `wakeup`
   - `clients` = empty (application handles fan-out)
4. Publish to `{clientId}/notify` for **every currently connected client**.

> **Age computation:** The age must be computed at the time the alarm fires, not at config load time. This ensures the displayed age is correct even if the application runs continuously across year boundaries.

**January 1st birthdays:** If a person's birthday falls on January 1st, both their birthday alarm and the New Year alarm fire at `00:00:00`. Both notifications are sent independently; neither is suppressed. The delivery order between them is not guaranteed. This is considered correct behavior — both events deserve celebration.

---

### 6.5 New Year Notification

**Purpose:** Display a "Happy New Year" message on all connected displays at the stroke of midnight on January 1st.

**Scheduling:** If `new_year.enabled` is `true`, schedule a repeating yearly alarm that fires at `00:00:00` in the configured timezone on January 1st.

**At alarm fire time:**

1. If `new_year.enabled` is `false`, do nothing and return.
2. Read the current year at the moment of firing (this is the year being welcomed in).
3. Build the notification text:
   - If `message` is empty or absent: generate `"Happy New Year <YYYY>!"` where `<YYYY>` is the current year.
   - If `message` is explicitly set: use it verbatim.
4. Build a `Notification` with:
   - `text` = computed or configured message
   - `icon` = configured `icon`
   - `duration` = configured `duration`
   - `rainbow` = configured `rainbow`
   - `scrollSpeed` = configured `scroll_speed`
   - `wakeup` = configured `wakeup`
   - `clients` = empty (application handles fan-out)
5. Publish to `{clientId}/notify` for **every currently connected client**.

> **Year computation:** The year in the message is computed at fire time, not at application startup or config load time.

---

### 6.6 Client State Tracking

The application maintains in-memory state for each connected client. This state is ephemeral — it is not written to disk and is lost on restart.

| State | Updated by | Description |
|---|---|---|
| Current app name | `{clientId}/stat/currentApp` publish | The app currently being displayed |
| Latest stats | `{clientId}/stats` publish | The most recently received `Stats` payload |

**Connection lifecycle:**

- On client **connect** (OnConnect hook): register the client as connected; push current settings to `{clientId}/settings`.
- On client **disconnect** (OnDisconnect hook): remove the client's in-memory state (stored stats and current app name) and remove it from the connected client set. This prevents stale state accumulation for devices that were once connected but are no longer reachable.

The application must be able to enumerate all **currently connected** client IDs at any point in time (used to fan out settings pushes and notifications).

---

### 6.7 Button Events

The application receives button events via the button topics. In the current feature set, button events are **received and logged but not acted upon**. The infrastructure for receiving them must be in place so future features can react to them.

| Button | Topic suffix |
|---|---|
| Left | `button/left` |
| Select | `button/select` |
| Right | `button/right` |

Payload `"1"` = pressed, `"0"` = released.

---

## 7. Scheduler & Timed Events

All scheduled times are interpreted in the configured timezone (see §2.2).

| Event | Schedule | Action |
|---|---|---|
| Sunrise | One-shot at today's sunrise; reschedules daily | Switch to Day mode; push settings to all clients |
| Sunset | One-shot at today's sunset; reschedules daily | Switch to Night mode; push settings to all clients |
| Energy saving start | Daily at configured `start` time | Activate energy-saving; push settings to all clients |
| Energy saving end | Daily at configured `end` time | Deactivate energy-saving; push settings to all clients |
| Birthday | Yearly on each person's month/day at `00:00:00` | Send birthday notification to all clients |
| New Year | Yearly on January 1st at `00:00:00` (if enabled) | Send New Year notification to all clients |

**Startup initialization order:**

1. Load and validate configuration; exit on error.
2. Determine current day/night state in the configured timezone; apply immediately.
3. Determine current energy-saving state; apply immediately.
4. Schedule the next sunrise/sunset transitions.
5. Schedule daily energy-saving start/end alarms.
6. Schedule all birthday alarms (one per birthday entry).
7. Schedule the New Year alarm (if `new_year.enabled` is `true`).
8. Start the MQTT broker and begin accepting connections.

**Missed events:** If the application starts after a scheduled event has already passed for the current day (e.g., it starts after sunrise), it must correctly determine the current state from the present time and schedule only the next future event. It must not fire the already-passed event.

---

## 8. Error Handling Requirements

| Situation | Required behavior |
|---|---|
| Config file absent or unreadable | Log an error with the file path; exit with code `1` |
| Config file present but invalid YAML | Log a parse error with line/column details; exit with code `1` |
| `mqtt.username` or `mqtt.password` absent | Log an error naming the missing field; exit with code `1` |
| `location.latitude` or `location.longitude` absent | Log an error; exit with code `1` |
| `timezone` absent | Log a warning that system timezone is being used; continue |
| `timezone` value is not a valid IANA name | Log an error; exit with code `1` |
| MQTT port already in use | Log a clear error including the port number; exit with code `2` |
| Client disconnects | Remove client state; cease sending it messages; no crash |
| Stats payload unparseable | Log a warning with the raw payload; discard; continue |
| Polar night/midnight sun (no sunrise or no sunset) | Log a warning; stay in last known mode; retry at next calendar day; do not crash or spin-retry |
| Color value with invalid format in config | Log a warning with the offending value; exit with code `1` (invalid config should not silently use a wrong color) |
| Birthday entry with invalid date | Log a warning identifying the entry; skip it; continue loading remaining entries |
| `new_year.enabled` absent | Default to `true`; no warning needed |
| MQTT publish fails | Log a warning with topic and error; do not retry automatically |
| Graceful shutdown signal (SIGTERM / SIGINT) | Stop the scheduler; stop the broker; clear state; exit with code `0` |

---

## 9. CLI Interface

The application is a single binary with a command-line interface.

### Usage

```
awtrix-controller [flags]
```

### Flags and Environment Variables

Each configurable flag has a corresponding environment variable. The resolution order is (highest priority first):

1. CLI flag (if explicitly provided)
2. Environment variable
3. Built-in default

| Flag | Short | Environment variable | Default | Description |
|---|---|---|---|---|
| `--config` | `-c` | `AWTRIX_CONFIG` | `/etc/awtrix-controller/config.yaml` | Path to the YAML configuration file |
| `--log-level` | `-l` | `AWTRIX_LOG_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `--version` | `-v` | — | — | Print version string and exit |

**Examples:**

```bash
# Via CLI flags
awtrix-controller --config /home/pi/awtrix.yaml --log-level debug

# Via environment variables (useful in Docker / systemd)
AWTRIX_CONFIG=/home/pi/awtrix.yaml AWTRIX_LOG_LEVEL=debug awtrix-controller

# Mixed (CLI flag takes precedence over env var)
AWTRIX_LOG_LEVEL=debug awtrix-controller --log-level info   # → info wins
```

### Exit Codes

| Code | Meaning |
|---|---|
| `0` | Clean shutdown |
| `1` | Configuration error (missing required field, unparseable YAML, invalid values) |
| `2` | Runtime startup error (port in use, etc.) |

### Logging

- Log output goes to **stdout**.
- Structured logging (JSON or logfmt) is recommended so log aggregators can parse it.
- Each log line must include at minimum: timestamp, level, and message.
- Log on startup: configured MQTT port, WebSocket port (if enabled), location, timezone, energy-saving window.
- Log on each mode change: new mode name and the number of clients notified.
- Log on each client connect/disconnect: client ID.

---

## 10. Deployment

### Docker

The application should be distributable as a minimal container image. Recommended approach:

- **Base image:** A minimal image (e.g., `gcr.io/distroless/static` or `alpine`) — no shell required in production.
- **Run as non-root user.**
- **Expose:** Port `1883` (TCP). Expose `ws_port` if WebSocket is enabled.
- **Configuration:** Mount the config file at the path given by `--config` or `AWTRIX_CONFIG`.
- **Persistent storage:** None required. The application maintains ephemeral in-memory state only; no disk writes occur during normal operation.

Example minimal Compose setup:

```yaml
services:
  awtrix-controller:
    image: awtrix-controller:latest
    ports:
      - "1883:1883"
    volumes:
      - ./config.yaml:/etc/awtrix-controller/config.yaml:ro
    environment:
      AWTRIX_LOG_LEVEL: info       # override log level without changing the config file
      # AWTRIX_CONFIG: /alt/path/config.yaml   # alternative config path if needed
    restart: unless-stopped
```

### Systemd

A systemd unit file should be provided for bare-metal / Raspberry Pi deployments:

```ini
[Unit]
Description=Awtrix3 MQTT Controller
After=network.target

[Service]
ExecStart=/usr/local/bin/awtrix-controller --config /etc/awtrix-controller/config.yaml
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### Resource Profile

This application is designed to run on low-power hardware (e.g., Raspberry Pi Zero 2). Memory usage should stay well under 64 MB resident. CPU should be near zero between events.

---

## 11. Security

### Intended Deployment Model

This application is designed for **trusted local area networks** only. It does not implement TLS or per-client authorization. It must not be exposed to the internet.

### Authentication

- MQTT credentials (username and password) must be supplied via configuration — **no hardcoded credentials**.
- Any client that fails to authenticate must be immediately disconnected.

### Future Considerations (out of scope for v1)

The following are not required but should not be architecturally precluded:

- TLS/mTLS for MQTT connections
- Per-client topic ACLs
- Configuration hot-reload

---

## 12. Behavioral Test Cases

The following scenarios describe expected end-to-end behavior and can serve as integration or acceptance tests.

**Prerequisite for time-dependent tests:** Any test that asserts behavior based on time of day, day/night state, energy-saving state, or scheduled alarm firing (TC-02 through TC-12) requires the implementation to support an injectable or mockable clock. The production implementation should accept a clock interface; tests substitute a controllable fake clock. Tests that rely on real wall-clock time are non-deterministic and must not be used as acceptance criteria.

---

### TC-01: Device connects and receives settings

1. Configure the application with a known location and timezone.
2. Fix the clock to a known time within the day period and outside the energy-saving window.
3. Start the application.
4. Connect a MQTT client with correct credentials.
5. **Expected:** The broker has a retained message on `{clientId}/settings`. When the client subscribes to that topic, it receives a `Settings` JSON with theme colors from `theme.day` and `ABRI=true` (no `BRI` field).

---

### TC-02: Day/Night transition

1. Fix the clock to just before sunset.
2. Start the application; verify it is in Day mode.
3. Advance the clock past sunset.
4. **Expected:** The application publishes a `Settings` message (retained) to all connected clients with `TIME_COL`, `DATE_COL`, `CBCOL`, `WDCI` set to the night `content` color.

---

### TC-03: Energy-saving activation

1. Configure `energy_saving.start: "02:00"`, `end: "06:00"`. Fix clock to `01:59`.
2. Start the application; verify energy saving is inactive.
3. Advance clock to `02:00`.
4. **Expected:** Settings push to all clients with `BRI=1` and `ABRI=false`.

---

### TC-04: Energy-saving deactivation

1. Continue from TC-03. Advance clock to `06:00`.
2. **Expected:** Settings push to all clients with `ABRI=true`; `BRI` field is absent from the payload.

---

### TC-05: Birthday alarm fires

1. Configure a birthday for today's date (month/day), birth year 1990. Fix clock to `1990-04-16` + 34 years = `2024-04-16 00:00:00`.
2. Advance clock to `00:00:00` on that date.
3. **Expected:** A `Notification` is published to all connected clients with `text = "Happy 34 Birthday <name>!"` and the configured icon, rainbow, rtttl, etc.

---

### TC-06: Birthday age is computed at fire time, not load time

1. Configure a birthday for March 3, 1990. Fix clock to December 1 of year N. Start application.
2. Advance clock to March 3, `00:00:00` of year N+1.
3. **Expected:** The notification text contains age `(N+1) - 1990`, not the age that would have been computed in December of year N.

---

### TC-07: Invalid credentials are rejected

1. Start the application.
2. Connect a MQTT client with incorrect credentials.
3. **Expected:** The client is disconnected immediately; no topics are accessible to it.

---

### TC-08: Stats are stored per client

1. Connect two clients with different client IDs.
2. Have each publish a `{clientId}/stats` payload.
3. **Expected:** The application stores separate stat records for each client ID; updating one does not affect the other.

---

### TC-09: Energy-saving window spanning midnight

1. Configure `energy_saving.start: "23:00"`, `end: "05:00"`. Fix clock to `22:59`.
2. Start application; verify energy saving is inactive.
3. Advance clock to `23:30`. **Expected:** Energy saving active (`BRI=1`, `ABRI=false`).
4. Advance clock to `04:59`. **Expected:** Energy saving still active.
5. Advance clock to `05:01`. **Expected:** Energy saving deactivates (`ABRI=true`, no `BRI`).

---

### TC-10: Graceful shutdown

1. Run the application with a connected client.
2. Send SIGTERM.
3. **Expected:** Application stops the scheduler, stops the broker, logs a shutdown message, and exits with code `0`.

---

### TC-11: New Year notification fires

1. Fix clock to `2024-12-31 23:59:59`. Start application with `new_year.enabled: true`.
2. Advance clock to `2025-01-01 00:00:00`.
3. **Expected:** A `Notification` is published to all connected clients with `text = "Happy New Year 2025!"`, `icon = "5855"` (or configured override), `duration = 600`, `rainbow = true`, `wakeup = true`.

---

### TC-12: New Year year is computed at fire time

1. Start the application in November of year N.
2. Advance clock to `00:00:00` on January 1st of year N+1.
3. **Expected:** Notification text contains `N+1`, not `N`.

---

### TC-13: New Year notification can be disabled

1. Configure `new_year.enabled: false`.
2. Advance clock to January 1st `00:00:00`.
3. **Expected:** No New Year notification is published.

---

### TC-14: New Year notification is configurable

1. Configure `new_year.icon: "1234"`, `new_year.message: "Felice Anno Nuovo!"`.
2. Advance clock to January 1st `00:00:00`.
3. **Expected:** Notification has `icon = "1234"` and `text = "Felice Anno Nuovo!"`.

---

### TC-15: January 1st birthday sends both notifications

1. Configure a birthday with `date_of_birth: "1990-01-01"`.
2. Advance clock to January 1st `00:00:00`.
3. **Expected:** Two separate `Notify` publishes are made to connected clients — one birthday notification and one New Year notification. Neither suppresses the other.

---

### TC-16: Client disconnect clears state

1. Connect a client; let it publish `/stats` and `/stat/currentApp`.
2. Disconnect the client.
3. **Expected:** The application's in-memory state for that client ID is cleared. If the application were to enumerate connected clients, the disconnected client ID would not appear.

---

### TC-17: Polar night fallback

1. Configure a location within the polar circle during winter (e.g., latitude 71°N in January).
2. Mock the sunrise computation to return "no result."
3. **Expected:** Application logs a warning (not an error), stays in Night mode, and schedules a retry for the next calendar day. No crash, no spin-retry.

---

### TC-18: Missing config file causes fatal exit

1. Point `--config` at a file that does not exist.
2. **Expected:** Application logs an error identifying the missing file and exits with code `1`.

---

### TC-19: Environment variable sets log level

1. Set `AWTRIX_LOG_LEVEL=debug` in the environment.
2. Start the application without any `--log-level` flag.
3. **Expected:** Application runs at debug log verbosity.

---

### TC-20: CLI flag takes precedence over environment variable

1. Set `AWTRIX_LOG_LEVEL=debug` in the environment.
2. Start the application with `--log-level warn`.
3. **Expected:** Application runs at warn verbosity (CLI wins).

---

### TC-21: Timezone controls alarm scheduling

1. Configure `timezone: "America/New_York"` and `energy_saving.start: "02:00"`.
2. Fix the system clock to `07:00 UTC` (= `02:00 EST`).
3. **Expected:** Energy-saving activates at this moment, not at `02:00 UTC`.
