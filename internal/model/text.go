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

import (
	"encoding/json"
	"fmt"
)

// TextFragment is one colored segment in a fragmented text value.
// Color uses a 6-digit hex string without the "#" prefix (e.g. "FF0000"),
// matching the firmware's convention for this field.
type TextFragment struct {
	Text  string `json:"t"`
	Color string `json:"c"`
}

// TextContent holds the value of the firmware "text" field, which accepts
// either a plain string or an array of colored TextFragment values.
// Use NewPlainText or NewFragmentedText to construct; use *TextContent in AppContent.
type TextContent struct {
	plain     string
	fragments []TextFragment
}

// NewPlainText returns a TextContent holding a plain string.
func NewPlainText(text string) *TextContent {
	return &TextContent{plain: text}
}

// NewFragmentedText returns a TextContent holding a slice of colored text fragments.
func NewFragmentedText(fragments ...TextFragment) *TextContent {
	return &TextContent{fragments: fragments}
}

// Plain returns the plain-text value, or empty string if this holds fragments.
func (textContent *TextContent) Plain() string { return textContent.plain }

// Fragments returns the fragment slice, or nil if this holds a plain string.
func (textContent *TextContent) Fragments() []TextFragment { return textContent.fragments }

// MarshalJSON encodes the value as a JSON string when plain, or a JSON array when fragmented.
func (textContent *TextContent) MarshalJSON() ([]byte, error) {
	if len(textContent.fragments) > 0 {
		return json.Marshal(textContent.fragments) //nolint:wrapcheck // implementing json.Marshaler
	}

	return json.Marshal(textContent.plain) //nolint:wrapcheck // implementing json.Marshaler
}

// UnmarshalJSON decodes a JSON string into a plain TextContent,
// or a JSON array into a fragmented TextContent.
func (textContent *TextContent) UnmarshalJSON(data []byte) error {
	if len(data) > 0 && data[0] == '"' {
		var plainValue string

		plainErr := json.Unmarshal(data, &plainValue)
		if plainErr != nil {
			return fmt.Errorf("text content (string): %w", plainErr)
		}

		textContent.plain = plainValue

		return nil
	}

	var fragmentsValue []TextFragment

	fragmentsErr := json.Unmarshal(data, &fragmentsValue)
	if fragmentsErr != nil {
		return fmt.Errorf("text content (fragments): %w", fragmentsErr)
	}

	textContent.fragments = fragmentsValue

	return nil
}
