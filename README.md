# awtrix-controller

A self-contained Go service that acts as the MQTT broker and automation controller
for [Awtrix3](https://blueforcer.github.io/awtrix3/) LED matrix devices.
Point your Awtrix3 devices at it and it handles day/night theming, energy saving,
scheduled notifications, and live weather overlays.

## What it does

`awtrix-controller` embeds a full MQTT broker (comqtt) that Awtrix3 devices connect
to directly. Once a device connects, the controller:

- Pushes the correct theme colors and display settings based on the current time of day.
- Dims or sleeps the display during a configurable energy-saving window.
- Fires scheduled notifications (birthdays, reminders, recurring messages) at the
  right moment.
- Fetches weather forecasts from [Open-Meteo](https://open-meteo.com/) (free, no API
  key) and applies overlay effects (rain, snow, thunderstorm, …) and sends severe-
  weather warning notifications.

## Key features

| Feature                 | Details                                                                  |
|-------------------------|--------------------------------------------------------------------------|
| Embedded MQTT broker    | TCP (default 1883) + optional WebSocket port                             |
| Day/night mode          | Sunrise/sunset calculated from configured coordinates                    |
| Energy-saving window    | Configurable HH:MM window, may span midnight                             |
| Theme management        | Day/night color sets pushed to every connected device                    |
| Scheduled notifications | `daily` / `weekly` / `monthly` / `yearly` recurrence                     |
| Weather overlays        | Open-Meteo forecast → overlay effect on all devices                      |
| Severe-weather alerts   | Push notifications for thunderstorm, frost, heavy rain, gusts, snow, fog |
| Multi-arch Docker image | `linux/amd64` and `linux/arm64` via GHCR                                 |

## Requirements

- Go 1.25+ (for building from source)
- Docker (for the container image)
- Awtrix3 firmware configured to connect to this broker's hostname/IP

## How to build

```bash
make go-build
```

Or with version metadata:

## How to run

### From source

```bash
# Copy and edit the sample config
cp deployments/config.sample.yaml config.yaml
$EDITOR config.yaml

# Run
./dist/awtrix-controller --config config.yaml --log-level debug
```

### With Docker Compose

```bash
# Place your config at /etc/awtrix-controller/config.yaml, then:
docker compose -f deployments/docker/docker-compose.yaml up -d
```

The Compose file uses the pre-built image from GHCR:

```
ghcr.io/leinardi/awtrix-controller:latest
```

### CLI flags

| Flag            | Short | Env var            | Default                              | Description                                               |
|-----------------|-------|--------------------|--------------------------------------|-----------------------------------------------------------|
| `--config`      | `-c`  | `AWTRIX_CONFIG`    | `/etc/awtrix-controller/config.yaml` | Path to YAML config file                                  |
| `--log-level`   | `-l`  | `AWTRIX_LOG_LEVEL` | `info`                               | Verbosity: `debug\|info\|warn\|error`                     |
| `--version`     | `-v`  | -                  | -                                    | Print version and exit                                    |
| `--weather-wmo` | -     | -                  | `0` (disabled)                       | Simulate all forecast points with a WMO code (debug only) |

## Configuration overview

See [`deployments/config.sample.yaml`](deployments/config.sample.yaml) for the full annotated example. Required fields are marked below.

### MQTT (required)

```yaml
mqtt:
  username: awtrix      # Awtrix3 devices must authenticate with this username
  password: changeme    # Change before deploying
  port: 1883            # TCP listen port (default: 1883)
  # ws_port: 8883       # WebSocket port; omit to disable
```

### Location (required)

Used for sunrise/sunset calculation. Also used by the weather feature.

```yaml
location:
  latitude: 52.5200
  longitude: 13.4050
```

### Timezone (strongly recommended)

IANA timezone name. Falls back to the system timezone with a warning when absent.

```yaml
timezone: Europe/Berlin
```

### Energy-saving window

Times are HH:MM in the configured timezone. The window may span midnight.

```yaml
energy_saving:
  start: "00:30"   # default
  end: "06:00"     # default
```

### Theme

Colors pushed to every connected device on day/night transitions (`#RRGGBB`).

```yaml
theme:
  day:
    calendar_accent: "#FF0000"
    content: "#FFFFFF"
  night:
    calendar_accent: "#FF0000"
    content: "#474747"
```

### Scheduled notifications

Repeating list of notifications. Supported `repeat` values: `daily`, `weekly`,
`monthly`, `yearly`.

```yaml
scheduled_notifications:
  - name: New Year
    message: "Happy New Year {year}!"
    repeat: yearly
    date: "01-01"          # MM-DD, required for yearly
    duration: 600          # seconds on screen (default: 60)
    icon: "5855"           # LaMetric icon ID (default: "9597")
    rainbow: true
    scroll_speed: 50
    wakeup: true

  - name: Weekly Standup
    message: "Standup in 15 minutes!"
    repeat: weekly
    weekdays: # required for weekly
      - monday
      - tuesday
      - wednesday
      - thursday
      - friday
    time: "09:45"          # HH:MM (default: "00:00")
```

Message templates: `{name}` expands to the notification `name`; `{year}` expands
to the current year.

### Weather

Polls Open-Meteo every `poll_interval_minutes`. No API key required.

```yaml
weather:
  enabled: true
  # poll_interval_minutes: 15
  # overlay_horizon_minutes: 60
  # notification_horizon_hours: 8
  # notification_repeat_minutes: 60
  # gust_warn_kmh: 45.0
  # gust_severe_kmh: 60.0
  # heavy_rain_mm_per_15min: 5.0
  # fog_visibility_warn_m: 1000.0
  # fog_visibility_severe_m: 200.0
  # frost_temp_c: 2.0
  # notify_thunderstorm: true
  # notify_freezing_precip: true
  # notify_frost_risk: true
  # notify_heavy_rain: true
  # notify_strong_gusts: true
  # notify_snow: true
  # notify_fog: false      # disabled by default (higher noise)
```

## Development commands

```bash
make go-build   # verify the binary compiles
make go-test    # run tests with the race detector
make check      # lint (golangci-lint) + tests
```

Run all three before submitting a change:

```bash
make go-build && make go-test && make check
```

## CI / Release

- **CI** (`.github/workflows/ci.yaml`): runs on every pull request - actionlint,
  pre-commit hooks (golangci-lint, hadolint, dclint, …), markdownlint, shellcheck,
  yamllint.
- **Release** (`.github/workflows/release.yaml`): triggered manually with a semver
  string. Builds static binaries for `linux/amd64` and `linux/arm64`, pushes a
  multi-arch Docker image to GHCR, and creates a GitHub Release with checksums.

## License

MIT - see [LICENSE](LICENSE) or the SPDX header in each source file.
Copyright © 2026 Roberto Leinardi.
