/*
 * MIT License
 *
 * Copyright (c) 2026 Roberto Leinardi
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

package model

// TransitionEffect selects the animation used when switching between apps (TEFF).
// The firmware default is TransitionEffectSlide.
type TransitionEffect int

const (
	// TransitionEffectRandom picks a random effect each transition.
	TransitionEffectRandom TransitionEffect = 0
	// TransitionEffectSlide slides the next app in from the right (firmware default).
	TransitionEffectSlide TransitionEffect = 1
	// TransitionEffectDim fades the display to black then back up.
	TransitionEffectDim TransitionEffect = 2
	// TransitionEffectZoom zooms in on the next app.
	TransitionEffectZoom TransitionEffect = 3
	// TransitionEffectRotate rotates the display during the transition.
	TransitionEffectRotate TransitionEffect = 4
	// TransitionEffectPixelate pixelates the display during the transition.
	TransitionEffectPixelate TransitionEffect = 5
	// TransitionEffectCurtain splits the display like a curtain.
	TransitionEffectCurtain TransitionEffect = 6
	// TransitionEffectRipple produces a ripple distortion during the transition.
	TransitionEffectRipple TransitionEffect = 7
	// TransitionEffectBlink blinks the display during the transition.
	TransitionEffectBlink TransitionEffect = 8
	// TransitionEffectReload reloads the display content during the transition.
	TransitionEffectReload TransitionEffect = 9
	// TransitionEffectFade fades between apps.
	TransitionEffectFade TransitionEffect = 10
)

// TimeFormat is a predefined time format string accepted by the TFORMAT field.
// The format tokens use strftime-style placeholders supported by the Awtrix3 firmware.
type TimeFormat string

const (
	// TimeFormat24hSeconds displays 24-hour time with seconds, e.g. "13:30:45".
	TimeFormat24hSeconds TimeFormat = "%H:%M:%S"
	// TimeFormat12hSeconds displays 12-hour time with seconds, e.g. "1:30:45".
	TimeFormat12hSeconds TimeFormat = "%l:%M:%S"
	// TimeFormat24h displays 24-hour time, e.g. "13:30".
	TimeFormat24h TimeFormat = "%H:%M"
	// TimeFormat24hBlinking displays 24-hour time with a blinking colon, e.g. "13:30".
	TimeFormat24hBlinking TimeFormat = "%H %M"
	// TimeFormat12h displays 12-hour time, e.g. "1:30".
	TimeFormat12h TimeFormat = "%l:%M"
	// TimeFormat12hBlinking displays 12-hour time with a blinking colon, e.g. "1:30".
	TimeFormat12hBlinking TimeFormat = "%l %M"
	// TimeFormat12hAMPM displays 12-hour time with AM/PM indicator, e.g. "1:30 PM".
	TimeFormat12hAMPM TimeFormat = "%l:%M %p"
	// TimeFormat12hBlinkingAMPM displays 12-hour time with a blinking colon and AM/PM, e.g. "1:30 PM".
	TimeFormat12hBlinkingAMPM TimeFormat = "%l %M %p"
)

// DateFormat is a predefined date format string accepted by the DFORMAT field.
type DateFormat string

const (
	// DateFormatDMY formats the date as Day.Month.Year (short), e.g. "16.04.22".
	DateFormatDMY DateFormat = "%d.%m.%y"
	// DateFormatDM formats the date as Day.Month, e.g. "16.04".
	DateFormatDM DateFormat = "%d.%m"
	// DateFormatYMD formats the date as Year-Month-Day, e.g. "22-04-16".
	DateFormatYMD DateFormat = "%y-%m-%d"
	// DateFormatMD formats the date as Month-Day, e.g. "04-16".
	DateFormatMD DateFormat = "%m-%d"
	// DateFormatMDY formats the date as Month/Day/Year, e.g. "04/16/22".
	DateFormatMDY DateFormat = "%m/%d/%y"
	// DateFormatMDSlash formats the date as Month/Day, e.g. "04/16".
	DateFormatMDSlash DateFormat = "%m/%d"
	// DateFormatDMYSlash formats the date as Day/Month/Year, e.g. "16/04/22".
	DateFormatDMYSlash DateFormat = "%d/%m/%y"
	// DateFormatDMSlash formats the date as Day/Month, e.g. "16/04".
	DateFormatDMSlash DateFormat = "%d/%m"
	// DateFormatMDY2 formats the date as Month-Day-Year, e.g. "04-16-22".
	DateFormatMDY2 DateFormat = "%m-%d-%y"
)

// OverlayEffect is a named weather overlay applied globally to the matrix display.
// Used by both the Settings.Overlay (OVERLAY) and Notification.Overlay fields.
type OverlayEffect string

const (
	// OverlayEffectClear removes any active overlay.
	OverlayEffectClear OverlayEffect = "clear"
	// OverlayEffectSnow displays falling snowflakes over the content.
	OverlayEffectSnow OverlayEffect = "snow"
	// OverlayEffectRain displays a rain effect over the content.
	OverlayEffectRain OverlayEffect = "rain"
	// OverlayEffectDrizzle displays a light drizzle effect over the content.
	OverlayEffectDrizzle OverlayEffect = "drizzle"
	// OverlayEffectStorm displays a storm effect over the content.
	OverlayEffectStorm OverlayEffect = "storm"
	// OverlayEffectThunder displays a thunderstorm effect over the content.
	OverlayEffectThunder OverlayEffect = "thunder"
	// OverlayEffectFrost displays a frost effect over the content.
	OverlayEffectFrost OverlayEffect = "frost"
)

// PushIconBehavior controls how the icon moves relative to the scrolling text
// in a notification (pushIcon field).
type PushIconBehavior int

const (
	// PushIconStatic keeps the icon static while the text scrolls (default).
	PushIconStatic PushIconBehavior = 0
	// PushIconScroll scrolls the icon out with the text, then re-appears.
	PushIconScroll PushIconBehavior = 1
	// PushIconFixed keeps the icon fixed on the left while text scrolls past it.
	PushIconFixed PushIconBehavior = 2
)

// TextCaseMode controls the capitalisation applied to notification text (textCase field).
type TextCaseMode int

const (
	// TextCaseGlobalSetting inherits the device-level UPPERCASE setting (firmware default).
	TextCaseGlobalSetting TextCaseMode = 0
	// TextCaseUppercase forces the text to uppercase regardless of the device setting.
	TextCaseUppercase TextCaseMode = 1
	// TextCaseAsIs displays the text exactly as sent, without any case conversion.
	TextCaseAsIs TextCaseMode = 2
)

// LifetimeResetMode controls when the lifetime counter of a custom app resets
// (lifetimeMode field in CustomApp).
type LifetimeResetMode int

const (
	// LifetimeResetOnMessage resets the lifetime counter each time a new message is received (default).
	LifetimeResetOnMessage LifetimeResetMode = 0
	// LifetimeResetOnView resets the lifetime counter each time the app is displayed.
	LifetimeResetOnView LifetimeResetMode = 1
)

// Effect is the name of a built-in background animation supported by the firmware.
// Used by the AppContent.Effect and (via Hidden features) the global background layer.
// The firmware broadcasts all available effect names via MQTT on stats/effects after startup.
type Effect string

const (
	// EffectBrickBreaker plays a brick-breaker game animation.
	EffectBrickBreaker Effect = "BrickBreaker"
	// EffectCheckerboard animates a color-cycling checkerboard pattern.
	EffectCheckerboard Effect = "Checkerboard"
	// EffectColorWaves sweeps color waves across the matrix.
	EffectColorWaves Effect = "ColorWaves"
	// EffectFade pulses the matrix through a color fade cycle.
	EffectFade Effect = "Fade"
	// EffectFireworks launches animated fireworks.
	EffectFireworks Effect = "Fireworks"
	// EffectLookingEyes animates a pair of eyes looking around.
	EffectLookingEyes Effect = "LookingEyes"
	// EffectMatrix renders a Matrix-style falling-code rain effect.
	EffectMatrix Effect = "Matrix"
	// EffectMovingLine sweeps a colored line across the matrix.
	EffectMovingLine Effect = "MovingLine"
	// EffectPacifica simulates a gently rolling ocean surface.
	EffectPacifica Effect = "Pacifica"
	// EffectPingPong plays a Pong game animation.
	EffectPingPong Effect = "PingPong"
	// EffectPlasma renders a psychedelic plasma blob animation.
	EffectPlasma Effect = "Plasma"
	// EffectPlasmaCloud renders a softer, cloud-like plasma animation.
	EffectPlasmaCloud Effect = "PlasmaCloud"
	// EffectRadar spins a radar sweep around the matrix.
	EffectRadar Effect = "Radar"
	// EffectRipple emanates concentric color ripples outward.
	EffectRipple Effect = "Ripple"
	// EffectSnake animates a snake moving around the matrix.
	EffectSnake Effect = "Snake"
	// EffectSwirlIn spirals colors inward toward the center.
	EffectSwirlIn Effect = "SwirlIn"
	// EffectSwirlOut spirals colors outward from the center.
	EffectSwirlOut Effect = "SwirlOut"
	// EffectTheaterChase scrolls a theater-marquee chase pattern.
	EffectTheaterChase Effect = "TheaterChase"
	// EffectTwinklingStars randomly twinkles pixels like stars.
	EffectTwinklingStars Effect = "TwinklingStars"
)

// EffectPalette is a built-in color palette used by EffectSettings.Palette.
// A palette defines 16 colors from which the firmware interpolates animation hues.
type EffectPalette string

const (
	// EffectPaletteCloud cycles through soft white and grey tones.
	EffectPaletteCloud EffectPalette = "Cloud"
	// EffectPaletteForest cycles through earthy green and brown tones.
	EffectPaletteForest EffectPalette = "Forest"
	// EffectPaletteHeat cycles through red, orange, and yellow tones.
	EffectPaletteHeat EffectPalette = "Heat"
	// EffectPaletteLava cycles through deep red and orange tones.
	EffectPaletteLava EffectPalette = "Lava"
	// EffectPaletteOcean cycles through blue and teal tones.
	EffectPaletteOcean EffectPalette = "Ocean"
	// EffectPaletteParty cycles through bright, saturated party colors.
	EffectPaletteParty EffectPalette = "Party"
	// EffectPaletteRainbow cycles through all hues of the rainbow.
	EffectPaletteRainbow EffectPalette = "Rainbow"
	// EffectPaletteStripe alternates between contrasting color bands.
	EffectPaletteStripe EffectPalette = "Stripe"
)

// BuiltInApp is the name of a native firmware app used with AppSwitch.
type BuiltInApp string

const (
	// BuiltInAppTime is the native clock/time app.
	BuiltInAppTime BuiltInApp = "Time"
	// BuiltInAppDate is the native date app.
	BuiltInAppDate BuiltInApp = "Date"
	// BuiltInAppTemperature is the native temperature app.
	BuiltInAppTemperature BuiltInApp = "Temperature"
	// BuiltInAppHumidity is the native humidity app.
	BuiltInAppHumidity BuiltInApp = "Humidity"
	// BuiltInAppBattery is the native battery app.
	BuiltInAppBattery BuiltInApp = "Battery"
)

// ClockMode selects the clock face layout used by the native time app (TMODE setting).
type ClockMode int

const (
	// ClockModeWeekdayBar displays the time with a weekday indicator bar at the bottom.
	ClockModeWeekdayBar ClockMode = 0
	// ClockModeCalendar displays the time with a weekday bar and a calendar box
	// highlighting the current day of the month. Weekday bar is at the bottom.
	ClockModeCalendar ClockMode = 1
	// ClockModeCalendarTop is the same as ClockModeCalendar but places the weekday bar
	// at the top of the matrix.
	ClockModeCalendarTop ClockMode = 2
	// ClockModeCalendarAlt displays the time with a weekday bar and an alternate
	// calendar icon style. Weekday bar is at the bottom.
	ClockModeCalendarAlt ClockMode = 3
	// ClockModeCalendarAltTop is the same as ClockModeCalendarAlt but places the
	// weekday bar at the top of the matrix.
	ClockModeCalendarAltTop ClockMode = 4
	// ClockModeBigTime displays the time in a large font, optionally with a 32x8
	// animated GIF (bigtime.gif) in the background.
	ClockModeBigTime ClockMode = 5
	// ClockModeBinary displays the time in binary format: top row = hours,
	// middle row = minutes, bottom row = seconds (each row has six dots).
	ClockModeBinary ClockMode = 6
)
