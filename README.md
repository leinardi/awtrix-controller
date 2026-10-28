# awtrix-controller

A self-contained Rust service that acts as the MQTT broker and automation controller for
[Awtrix3](https://blueforcer.github.io/awtrix3/) LED matrix devices. Point your Awtrix3 devices at
it and it handles day/night theming, energy saving, scheduled notifications, and live weather
overlays.

## What it does

`awtrix-controller` embeds a full MQTT broker ([rmqtt](https://github.com/rmqtt/rmqtt)) that
Awtrix3 devices connect to directly. Once a device connects, the controller:

- Pushes the correct theme colors and display settings based on the current time of day.
- Dims or sleeps the display during a configurable energy-saving window.
- Fires scheduled notifications (birthdays, reminders, recurring messages) at the right moment.
- Fetches weather forecasts from [Open-Meteo](https://open-meteo.com/) (free, no API key) and
  applies overlay effects (rain, snow, thunderstorm, …) and sends severe-weather warning
  notifications.

## Key features

| Feature | Details |
| --- | --- |
| Embedded MQTT broker | TCP (default 1883) + optional WebSocket port |
| Day/night mode | Sunrise/sunset calculated from configured coordinates |
| Energy-saving window | Configurable HH:MM window, may span midnight |
| Theme management | Day/night color sets pushed to every connected device |
| Scheduled notifications | `daily` / `weekly` / `monthly` / `yearly` recurrence |
| Weather overlays | Open-Meteo forecast → overlay effect on all devices |
| Severe-weather alerts | Push notifications for thunderstorm, frost, heavy rain, gusts, snow, fog |
| Multi-arch Docker image | `linux/amd64` and `linux/arm64` via GHCR |

## Requirements

- Rust 1.98 (for building from source; `rust-toolchain.toml` pins `1.98.0`, which rustup installs
  on first use)
- Docker (for the container image)
- Awtrix3 firmware configured to connect to this broker's hostname/IP

## Binaries and image

Each release attaches two static binaries, `awtrix-controller-linux-amd64` and
`awtrix-controller-linux-arm64`, with `checksums.txt`. They are linked against musl with
[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild), so they have no runtime
dependencies: no libc, no zoneinfo (the time-zone database is compiled in) and no CA files (the
HTTPS roots for Open-Meteo are compiled in too).

The image `ghcr.io/leinardi/awtrix-controller` is multi-arch (`linux/amd64`, `linux/arm64`): the
static binary on the distroless `dhi.io/static` base, about 8 MB. Its
entrypoint is the binary, so any arguments given to the container are flags. See
[docs/release.md](docs/release.md) for how a release is built, scanned, signed and attested.

## How to build

```bash
cargo build --release --locked   # ./target/release/awtrix-controller, for the host
```

The version strings that `--version` prints come from the environment at build time:
`AWTRIX_CONTROLLER_VERSION`, `AWTRIX_CONTROLLER_COMMIT` and `AWTRIX_CONTROLLER_DATE` (`dev`, `none`
and `unknown` when unset). `make rust-run` sets all three from git.

A static musl binary, as the releases ship it, needs [zig](https://ziglang.org/) and
cargo-zigbuild, plus the target's standard library (`rustup target add x86_64-unknown-linux-musl`):

```bash
cargo zigbuild --release --locked --target x86_64-unknown-linux-musl
```

The container image builds both itself, so `make docker-build` needs only Docker (and a `dhi.io`
login for the base images).

## How to run

### From source

```bash
# Copy and edit the sample config
cp deployments/config.sample.yaml config.yaml
$EDITOR config.yaml

# Run, with the version strings set from git
make rust-run ARGS="--config config.yaml --log-level debug"

# Or run a release build directly
./target/release/awtrix-controller --config config.yaml --log-level debug
```

### With Docker Compose

```bash
# Place your config at /etc/awtrix-controller/config.yaml, then:
docker compose -f deployments/docker/docker-compose.yaml up -d
```

The Compose file uses the pre-built image from GHCR:

```text
ghcr.io/leinardi/awtrix-controller:latest
```

### CLI flags

| Flag | Short | Env var | Default | Description |
| --- | --- | --- | --- | --- |
| `--config` | `-c` | `AWTRIX_CONFIG` | `/etc/awtrix-controller/config.yaml` | Path to YAML config file |
| `--log-level` | `-l` | `AWTRIX_LOG_LEVEL` | `info` | Verbosity: `debug\|info\|warn\|error` |
| `--log-format` | - | - | the config's `log_format`, else `text` | Log format: `text\|json` |
| `--version` | `-v` | - | - | Print version, commit and build date, and exit |
| `--weather-wmo` | - | - | `0` (disabled) | Simulate all forecast points with a WMO code (debug only) |
| `--help` | `-h` | - | - | Print the help and exit |

A flag wins over its environment variable, which is read only when the flag is absent. Long flags
take two dashes, with the value after a space or an `=` (`--config config.yaml`,
`--config=config.yaml`). A boolean flag alone means `true`, and takes a value only after an `=`
(`--test-notification-rainbow=false`), spelled `1`, `t`, `T`, `true`, `True`, `TRUE` or `0`, `f`,
`F`, `false`, `False`, `FALSE`. Integer flags take Go-style literals, as 1.x did: decimal, or `0x`,
`0o` or `0b` prefixed, and a leading `0` means octal (`010` is 8); `--test-notification-repeat` is
decimal only. A repeated flag keeps its last value. Positional arguments are an error.

The exit code is `0` after a clean shutdown (`SIGINT` or `SIGTERM`), `--version` or `--help`; `1`
for invalid flags or configuration; and `2` when the broker cannot start (a port in use, for
example). On a signal the controller stops within 2 seconds, even with a weather fetch in flight.

#### Test notification flags (debug only)

Pass any combination of `--test-notification-*` flags to send a one-shot notification to
every connected device as soon as it becomes ready. All fields are optional — only set the
ones you want to test.

| Flag | Type | Description |
| --- | --- | --- |
| `--test-notification-text` | string | Message text |
| `--test-notification-icon` | string | LaMetric icon ID |
| `--test-notification-duration` | int | Display duration in seconds |
| `--test-notification-rainbow` | bool | Cycle rainbow colors on the text |
| `--test-notification-scroll-speed` | int | Scroll speed in pixels per frame |
| `--test-notification-no-scroll` | bool | Display text statically (no scroll) |
| `--test-notification-color` | string | Text color as hex, e.g. `#FF0000` |
| `--test-notification-background` | string | Background fill color as hex |
| `--test-notification-overlay` | string | Weather overlay: `clear\|snow\|rain\|drizzle\|storm\|thunder\|frost` |
| `--test-notification-effect` | string | Background animation name, e.g. `Plasma` |
| `--test-notification-blink-text` | int | Blink rate in milliseconds |
| `--test-notification-fade-text` | int | Fade rate in milliseconds |
| `--test-notification-text-case` | `0\|1\|2` | `0`=global `1`=uppercase `2`=as-is |
| `--test-notification-top-text` | bool | Display text at the top of the matrix |
| `--test-notification-text-offset` | int | Horizontal text offset in pixels |
| `--test-notification-push-icon` | `0\|1\|2` | Icon behavior: `0`=static `1`=scroll `2`=fixed |
| `--test-notification-center` | `true\|false` | Center text (any other value, or omitted: device default) |
| `--test-notification-hold` | bool | Keep notification visible until dismissed |
| `--test-notification-sound` | string | Sound file name on the device filesystem |
| `--test-notification-rtttl` | string | RTTTL melody string to play |
| `--test-notification-loop-sound` | bool | Loop sound while the notification is displayed |
| `--test-notification-stack` | `true\|false` | Queue behind current notification (any other value, or omitted: device default) |
| `--test-notification-wakeup` | bool | Wake the display from sleep before showing |
| `--test-notification-repeat` | int | Scroll repeat count (`-1`=infinite; a non-integer, or omitted: device default) |

Example — test an overlay with an RTTTL melody:

```bash
./target/release/awtrix-controller --config deployments/config.sample.yaml \
  --test-notification-text "Thunderstorm!" \
  --test-notification-overlay thunder \
  --test-notification-rtttl "Batman:d=16,o=5,b=180:8p,8b,8b,8b,2e." \
  --test-notification-icon "1234" \
  --test-notification-duration 30
```

## Configuration overview

See [`deployments/config.sample.yaml`](deployments/config.sample.yaml) for the full annotated
example. Required fields are marked below.

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

### Log format

`text` (the default) writes human-readable lines, `json` one JSON object per line. The
`--log-format` flag overrides it.

```yaml
log_format: json
```

### Energy-saving window

Times are HH:MM in the configured timezone, as wall-clock times: `07:00` means 07:00 local on every
day, daylight-saving days included. The window may span midnight.

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

A monthly `day` past the end of a month fires on its last day (day 31 fires on April 30), and a
yearly `02-29` fires on February 28 outside leap years. On a daylight-saving change, a time that
does not exist fires shifted forward by the gap (02:30 becomes 03:30), and a time that happens
twice fires once, the first time.

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
  # frost_temp_c: 1.5
  # notify_thunderstorm: true
  # notify_freezing_precip: true
  # notify_frost_risk: true
  # notify_heavy_rain: true
  # notify_strong_gusts: true
  # notify_snow: true
  # notify_fog: false      # disabled by default (higher noise)
```

An omitted threshold takes its default; any value given, `0` included, is used as is.

Polling intervals, horizons, the notification repeat interval, stale-data TTL and the missing-poll
count must be positive; `0` still selects the default. Negative values are configuration errors.
Cached warnings replay to a newly ready device only while both the notification repeat interval
and `data_stale_ttl_minutes` have not elapsed. The TTL measures the age of the fetch that produced
the cached batch, not the age of a later poll that did not notify.

When visibility is available, fog alerts use `fog_visibility_severe_m` and
`fog_visibility_warn_m`, even without a fog weather code. Visibility equal to a threshold does
not meet that threshold. With missing visibility, WMO 45 or 48 still produces a warning.

## Migrating from 1.x

Version 2.0 replaces the Go implementation with a Rust one. The configuration file, the MQTT
topics and the payload fields stay the same; these are the differences.

| # | Area | 1.x (Go) | 2.x (Rust) |
| --- | --- | --- | --- |
| Q1 | Weather fetch | a `null` in an Open-Meteo array read as `0` (a false fog alert from 0 m visibility, a `null` temperature as 0 °C) | `null` is a missing value: that hour is skipped by the checks that need it |
| Q2 | Config | an explicit `0` threshold (`frost_temp_c: 0`) was replaced by its default | an explicit `0` is honoured; thresholds are validated by range (finite, and all but `frost_temp_c` at least 0) |
| Q3 | Day/night | sunrise and sunset computed for the UTC date | computed for the local date in the configured time zone |
| Q4 | Monthly notification | day 29 to 31 rolled over into the next month | clamped to the last day of the month |
| Q5 | Yearly notification | `02-29` fired on March 1 in non-leap years | fires on February 28 |
| Q6 | Daylight saving in scheduled notifications | whatever Go's `time.Date` resolved, which differs per zone (see below) | a time in a gap fires shifted forward by the gap; a time in a fold fires at the first, earlier instant |
| Q7 | JSON payloads | `<`, `>` and `&` escaped as `\u003c`, `\u003e` and `\u0026` | written literally |
| Q8 | Logging | slog text (`time=… level=INFO msg=…`) | tracing text by default; `log_format: json` or `--log-format json` for JSON |
| Q9 | CLI | Go `flag` | clap; see below |
| Q10 | Energy-saving window on daylight-saving days | the time since local midnight was added, so `07:00` fired at 08:00 on a spring-forward day and an hour early on a fall-back day | start and end are wall-clock times on every day (see below) |
| Q11 | Weather interval validation | negative intervals, horizons and missing-poll counts were accepted | rejected as configuration errors; `0` still selects the default |
| Q12 | Weather warning controls | stale-data TTL and fog warning visibility were parsed but unused | cached notification replays obey the TTL; known visibility uses the configured fog thresholds |

**Q9, the command line.** The flags, their defaults and their environment variables are the same,
plus `--log-format`. What changed:

- The single-dash long form (`-config`, `-log-level=debug`) is refused with an error naming the
  `--` spelling. `-c` and `-l` stay.
- `--help` and `-h` print the help to stdout and exit `0` (1.x exited `1`).
- A positional argument is an error, exit `1` (1.x ignored it).
- `--test-notification-text-case` and `--test-notification-push-icon` accept only `0`, `1` and `2`,
  the values the firmware documents, and exit `1` on anything else (1.x sent any integer on).

**Q6, daylight-saving times in scheduled notifications.** The rows where 1.x and 2.x differ, in UTC,
the same for daily, weekly, monthly and yearly notifications. Berlin's spring gap (02:30 fires at
03:30 CEST), New York's fall fold (01:30 fires at the first 01:30, EDT) and every ordinary time
agree.

| Zone and date | Time | 1.x | 2.x |
| --- | --- | --- | --- |
| Europe/Berlin, 2026-10-25 (fold) | 02:30 | 01:30Z, the second 02:30 (CET) | 00:30Z, the first 02:30 (CEST) |
| America/New_York, 2026-03-08 (gap) | 02:30 | 06:30Z, 01:30 EST | 07:30Z, 03:30 EDT |
| America/New_York, daily, the day after the gap | 02:30 | 2026-03-09 05:30Z, 01:30 EDT: the time drifted an hour | 2026-03-09 06:30Z, 02:30 EDT |

**Q4 and Q5, for a notification at 09:00 UTC.**

| Repeat | Counted from | 1.x | 2.x |
| --- | --- | --- | --- |
| monthly, day 31 | 2026-04-15 | 2026-05-01 | 2026-04-30 |
| monthly, day 30 | 2026-02-01 | 2026-03-02 | 2026-02-28 |
| monthly, day 29 | 2026-02-01 | 2026-03-01 | 2026-02-28 |
| monthly, day 31 | 2026-01-31 10:00 | 2026-03-03 | 2026-02-28 |
| yearly, `02-29` | 2026-01-01 | 2026-03-01 | 2026-02-28 |

Leap years agree.

**Q10, the energy-saving window.** Each day's window is the pair of instants its start and end
resolve to, with the Q6 rules. A window whose start resolves at or after its end (`02:30`–`03:15`
on a spring-forward day: the start moves to 03:30) is empty: energy saving does not engage that
day, and the controller logs it and waits for the next day's window. A restart decides from the
same instants, so it lands in the state continuous operation would have reached. Examples, start
and end in UTC:

| Zone and date | Window | 1.x | 2.x |
| --- | --- | --- | --- |
| Europe/Berlin, 2026-03-29 (spring) | 07:00–09:00 | 06:00–08:00 (08:00–10:00 local) | 05:00–07:00 |
| Europe/Berlin, 2026-03-29 | 02:30–03:15 | 01:30–02:15 | empty |
| Europe/Berlin, 2026-10-25 (fall) | 07:00–09:00 | 05:00–07:00 (06:00–08:00 local) | 06:00–08:00 |
| America/New_York, 2026-03-08 (spring) | 07:00–09:00 | 12:00–14:00 | 11:00–13:00 |
| America/New_York, 2026-11-01 (fall) | 07:00–09:00 | 11:00–13:00 | 12:00–14:00 |
| America/New_York, 2026-11-01 (fall) | 01:00–02:30 | 05:00–06:30 (01:30 EST local) | 05:00–07:30 |

Every other day agrees. On a transition day, 1.x was an hour off for every boundary after the
change, a 23:00 start included; a boundary before the change agrees, and so, in these examples, does
one inside the gap or the fold. New York's fall 02:30 is not inside its fold (there, 01:00–02:00
happens twice), so it comes after the change and is an hour off (the last row).

## Development commands

```bash
make verify     # build, rustfmt check, clippy and the tests (meta-tests included)
make check      # every pre-commit hook on every file
make audit-deps # cargo-deny against the RustSec advisories (network required)
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the prerequisites and the full list.

## CI / Release

- **CI** (`.github/workflows/ci.yaml`): on every pull request and every push to `main`, rustfmt,
  clippy, the tests and cargo-deny, every pre-commit hook on every file
  ([droast](https://github.com/immanuwell/dockerfile-roast), dclint, markdownlint, shellcheck,
  yamllint, …), and on pull requests a Conventional Commits check of every commit; `ci-result` is
  the one check that stands for all of them.
- **Release** (`.github/workflows/release.yaml`): dispatched by hand, with an optional version
  derived from the commits when left empty. It builds static binaries for `linux/amd64` and
  `linux/arm64` and a multi-arch image, scans the image with Trivy before tagging, then publishes
  the image to GHCR with provenance and a cosign signature, and the GitHub release with checksums.
  See [docs/release.md](docs/release.md).
- **Scheduled scan** (`.github/workflows/scheduled-scan.yaml`): the latest image and the Rust
  dependencies, every week.

See [CONTRIBUTING.md](CONTRIBUTING.md) to contribute, and [SECURITY.md](SECURITY.md) to report a
vulnerability.

## License

MIT - see [LICENSE](LICENSE) or the SPDX header in each source file.
Copyright © 2026 Roberto Leinardi.
