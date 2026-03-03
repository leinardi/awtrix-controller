/*
 * MIT License
 *
 * Copyright (c) 2025 Roberto Leinardi
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
// JSON field names match the firmware's exact UPPERCASE keys.
//
//nolint:tagliatelle // firmware uses UPPER_CASE JSON keys; struct field names are idiomatic Go
type Settings struct {
	// --- Fields managed by the application ---

	// ChCol is the calendar header color (#RRGGBB). Set from the active day/night theme.
	ChCol string `json:"CHCOL,omitempty"`
	// CbCol is the calendar background color (#RRGGBB). Set from the active day/night theme.
	CbCol string `json:"CBCOL,omitempty"`
	// Wdca is the weekday-active color (#RRGGBB). Set from the active day/night theme.
	Wdca string `json:"WDCA,omitempty"`
	// Wdci is the weekday-inactive color (#RRGGBB). Set from the active day/night theme.
	Wdci string `json:"WDCI,omitempty"`
	// TimeCol is the time display color (#RRGGBB). Set from the active day/night theme.
	TimeCol string `json:"TIME_COL,omitempty"`
	// DateCol is the date display color (#RRGGBB). Set from the active day/night theme.
	DateCol string `json:"DATE_COL,omitempty"`
	// Bri is the display brightness (1–255). Set only when energy saving is active (value 1).
	// Nil when energy saving is inactive so the field is absent from the JSON payload.
	Bri *int `json:"BRI,omitempty"`
	// Abri enables auto-brightness. Set to false when energy saving is active, true otherwise.
	// Pointer so that false is not suppressed by omitempty.
	Abri *bool `json:"ABRI,omitempty"`

	// --- Additional firmware fields (not managed by the application; included for completeness) ---

	Atime       int    `json:"ATIME,omitempty"`
	Teff        int    `json:"TEFF,omitempty"`
	Tspeed      int    `json:"TSPEED,omitempty"`
	Tcol        string `json:"TCOL,omitempty"`
	Tmode       int    `json:"TMODE,omitempty"`
	Ctcol       string `json:"CTCOL,omitempty"`
	Wd          bool   `json:"WD,omitempty"`
	Atrans      bool   `json:"ATRANS,omitempty"`
	Ccorrection []int  `json:"CCORRECTION,omitempty"`
	Ctemp       []int  `json:"CTEMP,omitempty"`
	Tformat     string `json:"TFORMAT,omitempty"`
	Dformat     string `json:"DFORMAT,omitempty"`
	Som         bool   `json:"SOM,omitempty"`
	Blockn      bool   `json:"BLOCKN,omitempty"`
	Uppercase   bool   `json:"UPPERCASE,omitempty"`
	TempCol     string `json:"TEMP_COL,omitempty"`
	HumCol      string `json:"HUM_COL,omitempty"`
	BatCol      string `json:"BAT_COL,omitempty"`
	Sspeed      int    `json:"SSPEED,omitempty"`
	Tim         bool   `json:"TIM,omitempty"`
	Dat         bool   `json:"DAT,omitempty"`
	ShowHum     bool   `json:"HUM,omitempty"`
	ShowTemp    bool   `json:"TEMP,omitempty"`
	ShowBat     bool   `json:"BAT,omitempty"`
	Matp        bool   `json:"MATP,omitempty"`
	Vol         int    `json:"VOL,omitempty"`
	Overlay     string `json:"OVERLAY,omitempty"`
}
