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
	"errors"
	"fmt"
)

const (
	drawPixelArgCount        = 3
	drawLineArgCount         = 5
	drawRectArgCount         = 5
	drawFilledRectArgCount   = 5
	drawCircleArgCount       = 4
	drawFilledCircleArgCount = 4
	drawTextArgCount         = 4
	drawBitmapArgCount       = 5
)

// DrawInstruction is implemented by all typed drawing commands.
// The unexported drawInstruction marker prevents external packages from satisfying
// this interface, approximating a sealed class.
type DrawInstruction interface {
	MarshalJSON() ([]byte, error)
	drawInstruction()
}

// DrawList is an ordered sequence of DrawInstruction values that marshals to/from
// the JSON array expected by the firmware: [{"dp":[x,y,cl]}, {"dl":[...]}, ...].
type DrawList []DrawInstruction

// UnmarshalJSON implements json.Unmarshaler for DrawList.
func (list *DrawList) UnmarshalJSON(data []byte) error {
	var rawElements []json.RawMessage

	rawErr := json.Unmarshal(data, &rawElements)
	if rawErr != nil {
		return fmt.Errorf("draw list: %w", rawErr)
	}

	result := make(DrawList, len(rawElements))

	for elementIndex, rawElement := range rawElements {
		instruction, instructionErr := unmarshalDrawInstruction(rawElement)
		if instructionErr != nil {
			return fmt.Errorf("draw list element %d: %w", elementIndex, instructionErr)
		}

		result[elementIndex] = instruction
	}

	*list = result

	return nil
}

// DrawPixel draws a single pixel.
type DrawPixel struct {
	X, Y  int
	Color string
}

// NewDrawPixel creates a new DrawPixel instruction.
func NewDrawPixel(x, y int, color string) *DrawPixel {
	return &DrawPixel{X: x, Y: y, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawPixel.
func (pixel *DrawPixel) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dp": []any{pixel.X, pixel.Y, pixel.Color},
		},
	)
}

func (*DrawPixel) drawInstruction() {}

// DrawLine draws a line between two points.
type DrawLine struct {
	X0, Y0, X1, Y1 int
	Color          string
}

// NewDrawLine creates a new DrawLine instruction.
func NewDrawLine(x0, y0, x1, y1 int, color string) *DrawLine {
	return &DrawLine{X0: x0, Y0: y0, X1: x1, Y1: y1, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawLine.
func (line *DrawLine) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dl": []any{line.X0, line.Y0, line.X1, line.Y1, line.Color},
		},
	)
}

func (*DrawLine) drawInstruction() {}

// DrawRect draws a rectangle outline.
type DrawRect struct {
	X, Y, Width, Height int
	Color               string
}

// NewDrawRect creates a new DrawRect instruction.
func NewDrawRect(x, y, width, height int, color string) *DrawRect {
	return &DrawRect{X: x, Y: y, Width: width, Height: height, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawRect.
func (rect *DrawRect) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dr": []any{rect.X, rect.Y, rect.Width, rect.Height, rect.Color},
		},
	)
}

func (*DrawRect) drawInstruction() {}

// DrawFilledRect draws a filled rectangle.
type DrawFilledRect struct {
	X, Y, Width, Height int
	Color               string
}

// NewDrawFilledRect creates a new DrawFilledRect instruction.
func NewDrawFilledRect(x, y, width, height int, color string) *DrawFilledRect {
	return &DrawFilledRect{X: x, Y: y, Width: width, Height: height, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawFilledRect.
func (filledRect *DrawFilledRect) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"df": []any{
				filledRect.X,
				filledRect.Y,
				filledRect.Width,
				filledRect.Height,
				filledRect.Color,
			},
		},
	)
}

func (*DrawFilledRect) drawInstruction() {}

// DrawCircle draws a circle outline.
type DrawCircle struct {
	X, Y, Radius int
	Color        string
}

// NewDrawCircle creates a new DrawCircle instruction.
func NewDrawCircle(x, y, radius int, color string) *DrawCircle {
	return &DrawCircle{X: x, Y: y, Radius: radius, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawCircle.
func (circle *DrawCircle) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dc": []any{circle.X, circle.Y, circle.Radius, circle.Color},
		},
	)
}

func (*DrawCircle) drawInstruction() {}

// DrawFilledCircle draws a filled circle.
type DrawFilledCircle struct {
	X, Y, Radius int
	Color        string
}

// NewDrawFilledCircle creates a new DrawFilledCircle instruction.
func NewDrawFilledCircle(x, y, radius int, color string) *DrawFilledCircle {
	return &DrawFilledCircle{X: x, Y: y, Radius: radius, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawFilledCircle.
func (filledCircle *DrawFilledCircle) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dfc": []any{filledCircle.X, filledCircle.Y, filledCircle.Radius, filledCircle.Color},
		},
	)
}

func (*DrawFilledCircle) drawInstruction() {}

// DrawText draws text at a given position.
type DrawText struct {
	X, Y        int
	Text, Color string
}

// NewDrawText creates a new DrawText instruction.
func NewDrawText(x, y int, text, color string) *DrawText {
	return &DrawText{X: x, Y: y, Text: text, Color: color}
}

// MarshalJSON implements json.Marshaler for DrawText.
func (drawText *DrawText) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"dt": []any{drawText.X, drawText.Y, drawText.Text, drawText.Color},
		},
	)
}

func (*DrawText) drawInstruction() {}

// DrawBitmap draws a bitmap at a given position.
type DrawBitmap struct {
	X, Y, Width, Height int
	Pixels              []int
}

// NewDrawBitmap creates a new DrawBitmap instruction.
func NewDrawBitmap(x, y, width, height int, pixels []int) *DrawBitmap {
	return &DrawBitmap{X: x, Y: y, Width: width, Height: height, Pixels: pixels}
}

// MarshalJSON implements json.Marshaler for DrawBitmap.
func (bitmap *DrawBitmap) MarshalJSON() ([]byte, error) {
	return json.Marshal( //nolint:wrapcheck // positional array format; wrapping would lose context
		map[string]any{
			"db": []any{bitmap.X, bitmap.Y, bitmap.Width, bitmap.Height, bitmap.Pixels},
		},
	)
}

func (*DrawBitmap) drawInstruction() {}

//nolint:ireturn // returns sealed DrawInstruction interface implemented only within this package
func unmarshalDrawInstruction(data json.RawMessage) (DrawInstruction, error) {
	var rawMap map[string]json.RawMessage

	mapErr := json.Unmarshal(data, &rawMap)
	if mapErr != nil {
		return nil, fmt.Errorf("draw command: %w", mapErr)
	}

	if len(rawMap) != 1 {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"draw command: expected exactly 1 key, got %d",
			len(rawMap),
		)
	}

	for commandKey, rawArgs := range rawMap {
		var argElements []json.RawMessage

		argsErr := json.Unmarshal(rawArgs, &argElements)
		if argsErr != nil {
			return nil, fmt.Errorf("draw command %q args: %w", commandKey, argsErr)
		}

		switch commandKey {
		case "dp":
			return unmarshalDrawPixel(argElements)
		case "dl":
			return unmarshalDrawLine(argElements)
		case "dr":
			return unmarshalDrawRect(argElements)
		case "df":
			return unmarshalDrawFilledRect(argElements)
		case "dc":
			return unmarshalDrawCircle(argElements)
		case "dfc":
			return unmarshalDrawFilledCircle(argElements)
		case "dt":
			return unmarshalDrawText(argElements)
		case "db":
			return unmarshalDrawBitmap(argElements)
		default:
			return nil, fmt.Errorf( //nolint:err113 // dynamic message includes key name for diagnostics
				"draw command: unknown key %q",
				commandKey,
			)
		}
	}

	return nil, errors.New( //nolint:err113 // unreachable safety net after loop; static message is sufficient
		"draw command object is empty",
	)
}

func unmarshalDrawPixel(args []json.RawMessage) (*DrawPixel, error) {
	if len(args) != drawPixelArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dp: expected %d args, got %d",
			drawPixelArgCount,
			len(args),
		)
	}

	var xValue, yValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("dp: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("dp: arg 1 (y): %w", yErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[2], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dp: arg 2 (color): %w", colorErr)
	}

	return NewDrawPixel(xValue, yValue, colorValue), nil
}

//nolint:dupl // similar structure is intentional; each function targets a distinct draw command
func unmarshalDrawLine(args []json.RawMessage) (*DrawLine, error) {
	if len(args) != drawLineArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dl: expected %d args, got %d",
			drawLineArgCount,
			len(args),
		)
	}

	var x0Value, y0Value, x1Value, y1Value int

	x0Err := json.Unmarshal(args[0], &x0Value)
	if x0Err != nil {
		return nil, fmt.Errorf("dl: arg 0 (x0): %w", x0Err)
	}

	y0Err := json.Unmarshal(args[1], &y0Value)
	if y0Err != nil {
		return nil, fmt.Errorf("dl: arg 1 (y0): %w", y0Err)
	}

	x1Err := json.Unmarshal(args[2], &x1Value)
	if x1Err != nil {
		return nil, fmt.Errorf("dl: arg 2 (x1): %w", x1Err)
	}

	y1Err := json.Unmarshal(args[3], &y1Value)
	if y1Err != nil {
		return nil, fmt.Errorf("dl: arg 3 (y1): %w", y1Err)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[4], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dl: arg 4 (color): %w", colorErr)
	}

	return NewDrawLine(x0Value, y0Value, x1Value, y1Value, colorValue), nil
}

//nolint:dupl // similar structure is intentional; each function targets a distinct draw command
func unmarshalDrawRect(args []json.RawMessage) (*DrawRect, error) {
	if len(args) != drawRectArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dr: expected %d args, got %d",
			drawRectArgCount,
			len(args),
		)
	}

	var xValue, yValue, widthValue, heightValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("dr: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("dr: arg 1 (y): %w", yErr)
	}

	widthErr := json.Unmarshal(args[2], &widthValue)
	if widthErr != nil {
		return nil, fmt.Errorf("dr: arg 2 (width): %w", widthErr)
	}

	heightErr := json.Unmarshal(args[3], &heightValue)
	if heightErr != nil {
		return nil, fmt.Errorf("dr: arg 3 (height): %w", heightErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[4], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dr: arg 4 (color): %w", colorErr)
	}

	return NewDrawRect(xValue, yValue, widthValue, heightValue, colorValue), nil
}

//nolint:dupl // similar structure is intentional; each function targets a distinct draw command
func unmarshalDrawFilledRect(args []json.RawMessage) (*DrawFilledRect, error) {
	if len(args) != drawFilledRectArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"df: expected %d args, got %d",
			drawFilledRectArgCount,
			len(args),
		)
	}

	var xValue, yValue, widthValue, heightValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("df: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("df: arg 1 (y): %w", yErr)
	}

	widthErr := json.Unmarshal(args[2], &widthValue)
	if widthErr != nil {
		return nil, fmt.Errorf("df: arg 2 (width): %w", widthErr)
	}

	heightErr := json.Unmarshal(args[3], &heightValue)
	if heightErr != nil {
		return nil, fmt.Errorf("df: arg 3 (height): %w", heightErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[4], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("df: arg 4 (color): %w", colorErr)
	}

	return NewDrawFilledRect(xValue, yValue, widthValue, heightValue, colorValue), nil
}

//nolint:dupl // similar structure is intentional; each function targets a distinct draw command
func unmarshalDrawCircle(args []json.RawMessage) (*DrawCircle, error) {
	if len(args) != drawCircleArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dc: expected %d args, got %d",
			drawCircleArgCount,
			len(args),
		)
	}

	var xValue, yValue, radiusValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("dc: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("dc: arg 1 (y): %w", yErr)
	}

	radiusErr := json.Unmarshal(args[2], &radiusValue)
	if radiusErr != nil {
		return nil, fmt.Errorf("dc: arg 2 (radius): %w", radiusErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[3], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dc: arg 3 (color): %w", colorErr)
	}

	return NewDrawCircle(xValue, yValue, radiusValue, colorValue), nil
}

//nolint:dupl // similar structure is intentional; each function targets a distinct draw command
func unmarshalDrawFilledCircle(args []json.RawMessage) (*DrawFilledCircle, error) {
	if len(args) != drawFilledCircleArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dfc: expected %d args, got %d",
			drawFilledCircleArgCount,
			len(args),
		)
	}

	var xValue, yValue, radiusValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("dfc: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("dfc: arg 1 (y): %w", yErr)
	}

	radiusErr := json.Unmarshal(args[2], &radiusValue)
	if radiusErr != nil {
		return nil, fmt.Errorf("dfc: arg 2 (radius): %w", radiusErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[3], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dfc: arg 3 (color): %w", colorErr)
	}

	return NewDrawFilledCircle(xValue, yValue, radiusValue, colorValue), nil
}

func unmarshalDrawText(args []json.RawMessage) (*DrawText, error) {
	if len(args) != drawTextArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"dt: expected %d args, got %d",
			drawTextArgCount,
			len(args),
		)
	}

	var xValue, yValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("dt: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("dt: arg 1 (y): %w", yErr)
	}

	var textValue string

	textErr := json.Unmarshal(args[2], &textValue)
	if textErr != nil {
		return nil, fmt.Errorf("dt: arg 2 (text): %w", textErr)
	}

	var colorValue string

	colorErr := json.Unmarshal(args[3], &colorValue)
	if colorErr != nil {
		return nil, fmt.Errorf("dt: arg 3 (color): %w", colorErr)
	}

	return NewDrawText(xValue, yValue, textValue, colorValue), nil
}

func unmarshalDrawBitmap(args []json.RawMessage) (*DrawBitmap, error) {
	if len(args) != drawBitmapArgCount {
		return nil, fmt.Errorf( //nolint:err113 // dynamic message includes actual count for diagnostics
			"db: expected %d args, got %d",
			drawBitmapArgCount,
			len(args),
		)
	}

	var xValue, yValue, widthValue, heightValue int

	xErr := json.Unmarshal(args[0], &xValue)
	if xErr != nil {
		return nil, fmt.Errorf("db: arg 0 (x): %w", xErr)
	}

	yErr := json.Unmarshal(args[1], &yValue)
	if yErr != nil {
		return nil, fmt.Errorf("db: arg 1 (y): %w", yErr)
	}

	widthErr := json.Unmarshal(args[2], &widthValue)
	if widthErr != nil {
		return nil, fmt.Errorf("db: arg 2 (width): %w", widthErr)
	}

	heightErr := json.Unmarshal(args[3], &heightValue)
	if heightErr != nil {
		return nil, fmt.Errorf("db: arg 3 (height): %w", heightErr)
	}

	var pixelsValue []int

	pixelsErr := json.Unmarshal(args[4], &pixelsValue)
	if pixelsErr != nil {
		return nil, fmt.Errorf("db: arg 4 (pixels): %w", pixelsErr)
	}

	return NewDrawBitmap(xValue, yValue, widthValue, heightValue, pixelsValue), nil
}
