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

// Settings represents the partial settings payload sent to {clientId}/settings
// with retain=true. Every field uses omitempty so the application only transmits
// the fields it actively manages; device settings not listed here are left unchanged.
//
// Bri and Abri are pointer types because omitempty on a plain int or bool would
// suppress the zero value (0 or false), which are valid firmware values. A nil
// pointer is unambiguously "not set" and is correctly omitted.
//
// Several bool fields whose firmware default is true (e.g. Wd, Tim, Dat) are also
// pointer types so that an explicit false can be transmitted — a plain bool with
// omitempty would silently suppress the false value.
//
// JSON field names match the firmware's exact UPPERCASE keys.
//
//nolint:tagliatelle // firmware uses UPPER_CASE JSON keys; struct field names are idiomatic Go
type Settings struct {
	// --- Fields managed by the application ---

	// ChCol is the calendar header color (#RRGGBB). Maps to CHCOL. Default: "#FF0000".
	// Set from the active day/night theme.
	ChCol string `json:"CHCOL,omitempty"`
	// CbCol is the calendar body/background color (#RRGGBB). Maps to CBCOL. Default: "#FFFFFF".
	// Set from the active day/night theme.
	CbCol string `json:"CBCOL,omitempty"`
	// Wdca is the active weekday indicator color (#RRGGBB). Maps to WDCA.
	// Set from the active day/night theme.
	Wdca string `json:"WDCA,omitempty"`
	// Wdci is the inactive weekday indicator color (#RRGGBB). Maps to WDCI.
	// Set from the active day/night theme.
	Wdci string `json:"WDCI,omitempty"`
	// TimeCol is the text color of the time app (#RRGGBB). Maps to TIME_COL.
	// Use "#000000" (or omit) to inherit the global text color.
	// Set from the active day/night theme.
	TimeCol string `json:"TIME_COL,omitempty"`
	// DateCol is the text color of the date app (#RRGGBB). Maps to DATE_COL.
	// Use "#000000" (or omit) to inherit the global text color.
	// Set from the active day/night theme.
	DateCol string `json:"DATE_COL,omitempty"`
	// Bri is the matrix brightness (0–255). Maps to BRI.
	// Nil when energy saving is inactive so the field is absent from the JSON payload.
	// Set to 1 when energy saving is active.
	Bri *int `json:"BRI,omitempty"`
	// Abri enables automatic brightness control. Maps to ABRI.
	// Pointer so that false is not suppressed by omitempty.
	// Set to false when energy saving is active, true otherwise.
	Abri *bool `json:"ABRI,omitempty"`

	// --- Additional firmware fields (not managed by the application; included for completeness) ---

	// Atime is the duration each app is displayed in seconds. Maps to ATIME.
	// Range: positive integer. Default: 7.
	Atime int `json:"ATIME,omitempty"`
	// Teff selects the animation used when switching between apps. Maps to TEFF.
	// Range: TransitionEffectRandom(0)–TransitionEffectFade(10). Default: TransitionEffectSlide(1).
	// Pointer so that TransitionEffectRandom(0) is not suppressed by omitempty.
	Teff *TransitionEffect `json:"TEFF,omitempty"`
	// Tspeed is the duration of the transition animation in milliseconds. Maps to TSPEED.
	// Range: positive integer. Default: 500.
	Tspeed int `json:"TSPEED,omitempty"`
	// Tcol is the global text color. Maps to TCOL.
	// Value: RGB array (e.g. [255,0,0]) or hex color (e.g. "#FF0000").
	Tcol string `json:"TCOL,omitempty"`
	// Tmode selects the clock face style used by the time app. Maps to TMODE.
	// Use the ClockMode* constants (e.g. ClockModeCalendar). Default: ClockModeCalendar(1).
	// Pointer so that ClockModeWeekdayBar(0) is not suppressed by omitempty.
	Tmode *ClockMode `json:"TMODE,omitempty"`
	// Ctcol is the calendar text color in the time app (#RRGGBB). Maps to CTCOL. Default: "#000000".
	Ctcol string `json:"CTCOL,omitempty"`
	// Wd enables or disables the weekday display. Maps to WD. Default: true.
	// Pointer so that false is not suppressed by omitempty.
	Wd *bool `json:"WD,omitempty"`
	// Atrans enables automatic switching to the next app. Maps to ATRANS.
	// Pointer so that false is not suppressed by omitempty.
	Atrans *bool `json:"ATRANS,omitempty"`
	// Ccorrection applies color correction to the matrix as an RGB array. Maps to CCORRECTION.
	Ccorrection []int `json:"CCORRECTION,omitempty"`
	// Ctemp applies a color temperature adjustment to the matrix as an RGB array. Maps to CTEMP.
	Ctemp []int `json:"CTEMP,omitempty"`
	// Tformat sets the time format string for the time app. Maps to TFORMAT.
	// Use the TimeFormat* constants (e.g. TimeFormat24h). See available formats in enums.go.
	Tformat TimeFormat `json:"TFORMAT,omitempty"`
	// Dformat sets the date format string for the date app. Maps to DFORMAT.
	// Use the DateFormat* constants (e.g. DateFormatDMY). See available formats in enums.go.
	Dformat DateFormat `json:"DFORMAT,omitempty"`
	// Som starts the week on Monday when true. Maps to SOM. Default: true.
	// Pointer so that false (week starts on Sunday) is not suppressed by omitempty.
	Som *bool `json:"SOM,omitempty"`
	// Cel shows temperature in Celsius when true, Fahrenheit when false. Maps to CEL. Default: true.
	// Pointer so that false is not suppressed by omitempty.
	Cel *bool `json:"CEL,omitempty"`
	// Blockn blocks the physical navigation keys when true (they still send MQTT events). Maps to BLOCKN.
	// Default: false.
	Blockn bool `json:"BLOCKN,omitempty"`
	// Uppercase displays all text in uppercase. Maps to UPPERCASE. Default: true.
	// Pointer so that false is not suppressed by omitempty.
	Uppercase *bool `json:"UPPERCASE,omitempty"`
	// TempCol is the text color of the temperature app (#RRGGBB). Maps to TEMP_COL.
	// Use "#000000" (or omit) to inherit the global text color.
	TempCol string `json:"TEMP_COL,omitempty"`
	// HumCol is the text color of the humidity app (#RRGGBB). Maps to HUM_COL.
	// Use "#000000" (or omit) to inherit the global text color.
	HumCol string `json:"HUM_COL,omitempty"`
	// BatCol is the text color of the battery app (#RRGGBB). Maps to BAT_COL.
	// Use "#000000" (or omit) to inherit the global text color.
	BatCol string `json:"BAT_COL,omitempty"`
	// Sspeed adjusts the scroll speed as a percentage of the original speed. Maps to SSPEED.
	// Range: positive integer (percentage). Default: 100.
	Sspeed int `json:"SSPEED,omitempty"`
	// Tim enables or disables the native time app. Maps to TIM. Default: true.
	// Requires device reboot to take effect.
	// Pointer so that false is not suppressed by omitempty.
	Tim *bool `json:"TIM,omitempty"`
	// Dat enables or disables the native date app. Maps to DAT. Default: true.
	// Requires device reboot to take effect.
	// Pointer so that false is not suppressed by omitempty.
	Dat *bool `json:"DAT,omitempty"`
	// ShowHum enables or disables the native humidity app. Maps to HUM. Default: true.
	// Requires device reboot to take effect.
	// Pointer so that false is not suppressed by omitempty.
	ShowHum *bool `json:"HUM,omitempty"`
	// ShowTemp enables or disables the native temperature app. Maps to TEMP. Default: true.
	// Requires device reboot to take effect.
	// Pointer so that false is not suppressed by omitempty.
	ShowTemp *bool `json:"TEMP,omitempty"`
	// ShowBat enables or disables the native battery app. Maps to BAT. Default: true.
	// Requires device reboot to take effect.
	// Pointer so that false is not suppressed by omitempty.
	ShowBat *bool `json:"BAT,omitempty"`
	// Matp enables or disables the matrix display. Maps to MATP. Default: true.
	// Similar to the power endpoint but without the animation.
	// Pointer so that false is not suppressed by omitempty.
	Matp *bool `json:"MATP,omitempty"`
	// Vol sets the volume of the buzzer and DFPlayer. Maps to VOL.
	// Range: 0–30. Default: 0.
	Vol int `json:"VOL,omitempty"`
	// Overlay sets a global weather effect overlay. Maps to OVERLAY.
	// Nil serializes as JSON null, which clears any active overlay on the device.
	// Use the OverlayEffect* constants (e.g. OverlayEffectSnow) for active overlays.
	Overlay *OverlayEffect `json:"OVERLAY"`
}
