use base64::Engine as _;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::{CaptureFlowError, ProcessedCapture};

const PIN_COLOR: [u8; 4] = [255, 95, 86, 255];
const RECT_COLOR: [u8; 4] = [255, 179, 71, 255];
const LABEL_BG: [u8; 4] = [24, 24, 27, 235];
const LABEL_FG: [u8; 4] = [255, 255, 255, 255];
const LABEL_PAD_X: i32 = 4;
const LABEL_PAD_Y: i32 = 3;
const FONT_SCALE: i32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub kind: AnnotationKind,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AnnotationKind {
    Pin {
        x: f32,
        y: f32,
    },
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotatedImage {
    pub encoded_png: Vec<u8>,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Annotation {
    pub fn pin(index: usize, x: f32, y: f32, note: Option<String>) -> Self {
        Self {
            id: (index + 1).to_string(),
            kind: AnnotationKind::Pin {
                x: clamp_unit(x),
                y: clamp_unit(y),
            },
            note: sanitize_note(note),
        }
    }

    pub fn rectangle(
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        note: Option<String>,
    ) -> Self {
        Self {
            id: alpha_id(index),
            kind: AnnotationKind::Rectangle {
                x: clamp_unit(x),
                y: clamp_unit(y),
                width: clamp_unit(width),
                height: clamp_unit(height),
            },
            note: sanitize_note(note),
        }
    }

    pub fn center(&self) -> (f32, f32) {
        match self.kind {
            AnnotationKind::Pin { x, y } => (x, y),
            AnnotationKind::Rectangle {
                x,
                y,
                width,
                height,
            } => (
                clamp_unit(x + (width / 2.0)),
                clamp_unit(y + (height / 2.0)),
            ),
        }
    }
}

pub fn apply_annotations_to_processed_capture(
    mut processed: ProcessedCapture,
    annotations: &[Annotation],
) -> Result<ProcessedCapture, CaptureFlowError> {
    if annotations.is_empty() {
        return Ok(processed);
    }

    let annotated = render_annotations_to_png(
        &processed.rgba,
        processed.metadata.optimized_width,
        processed.metadata.optimized_height,
        annotations,
    )?;

    processed.metadata.file_size = annotated.encoded_png.len();
    processed.metadata.image_base64 =
        base64::engine::general_purpose::STANDARD.encode(&annotated.encoded_png);
    processed.encoded = annotated.encoded_png;
    processed.rgba = annotated.rgba;

    Ok(processed)
}

pub fn render_annotations_to_png(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    annotations: &[Annotation],
) -> Result<AnnotatedImage, CaptureFlowError> {
    let mut rgba = rgba_data.to_vec();
    let width_i32 = width as i32;
    let height_i32 = height as i32;

    for annotation in annotations {
        match annotation.kind {
            AnnotationKind::Pin { x, y } => {
                let px = (x * width as f32).round() as i32;
                let py = (y * height as f32).round() as i32;
                draw_pin(&mut rgba, width_i32, height_i32, px, py, &annotation.id);
            }
            AnnotationKind::Rectangle {
                x,
                y,
                width: rect_w,
                height: rect_h,
            } => {
                let px = (x * width as f32).round() as i32;
                let py = (y * height as f32).round() as i32;
                let rw = ((rect_w * width as f32).round() as i32).max(8);
                let rh = ((rect_h * height as f32).round() as i32).max(8);
                draw_rectangle(
                    &mut rgba,
                    width_i32,
                    height_i32,
                    px,
                    py,
                    rw,
                    rh,
                    &annotation.id,
                );
            }
        }
    }

    let image = image::RgbaImage::from_raw(width, height, rgba.clone())
        .ok_or(CaptureFlowError::InvalidRgbaDimensions { width, height })?;

    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(CaptureFlowError::EncodeImage)?;

    Ok(AnnotatedImage {
        encoded_png: cursor.into_inner(),
        rgba,
        width,
        height,
    })
}

pub fn build_llm_prompt_hint(annotations: &[Annotation]) -> Option<String> {
    build_llm_prompt_hint_for_language(annotations, "en")
}

pub fn build_llm_prompt_hint_for_language(
    annotations: &[Annotation],
    language: &str,
) -> Option<String> {
    if annotations.is_empty() {
        return None;
    }

    let mut lines = Vec::with_capacity(annotations.len() + 1);
    lines.push(prompt_intro(language).to_string());

    for annotation in annotations {
        let (cx, cy) = annotation.center();
        let pct_x = (cx * 100.0).round() as i32;
        let pct_y = (cy * 100.0).round() as i32;
        let area = region_label_for_language(cx, cy, language);
        match annotation.kind {
            AnnotationKind::Pin { .. } => {
                let note = annotation
                    .note
                    .as_deref()
                    .unwrap_or(default_pin_note(language));
                lines.push(format_pin_line(
                    language,
                    &annotation.id,
                    area,
                    pct_x,
                    pct_y,
                    note,
                ));
            }
            AnnotationKind::Rectangle {
                x,
                y,
                width,
                height,
            } => {
                let x1 = (x * 100.0).round() as i32;
                let y1 = (y * 100.0).round() as i32;
                let x2 = ((x + width) * 100.0).round() as i32;
                let y2 = ((y + height) * 100.0).round() as i32;
                let note = annotation
                    .note
                    .as_deref()
                    .unwrap_or(default_rect_note(language));
                lines.push(format_rect_line(
                    language,
                    &annotation.id,
                    area,
                    x1,
                    x2,
                    y1,
                    y2,
                    note,
                ));
            }
        }
    }

    Some(lines.join("\n"))
}

fn draw_pin(buffer: &mut [u8], width: i32, height: i32, x: i32, y: i32, label: &str) {
    let radius = 12;
    draw_line(buffer, width, height, x, y + 6, x, y + 30, PIN_COLOR, 3);
    draw_filled_circle(buffer, width, height, x, y, radius, PIN_COLOR);
    draw_circle_outline(buffer, width, height, x, y, radius + 1, LABEL_FG, 2);
    draw_label_chip(
        buffer,
        width,
        height,
        x + 14,
        y - 10,
        label,
        PIN_COLOR,
        LABEL_FG,
    );
}

fn draw_rectangle(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_w: i32,
    rect_h: i32,
    label: &str,
) {
    draw_rect_outline(buffer, width, height, x, y, rect_w, rect_h, RECT_COLOR, 3);
    draw_label_chip(
        buffer,
        width,
        height,
        x + 8,
        y + 8,
        label,
        RECT_COLOR,
        LABEL_FG,
    );
}

fn draw_label_chip(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    text: &str,
    accent: [u8; 4],
    text_color: [u8; 4],
) {
    let glyph_w = 6 * FONT_SCALE;
    let glyph_h = 7 * FONT_SCALE;
    let text_len = text.chars().count() as i32;
    let chip_w = LABEL_PAD_X * 2 + text_len * glyph_w;
    let chip_h = LABEL_PAD_Y * 2 + glyph_h;
    draw_filled_rect(buffer, width, height, x, y, chip_w, chip_h, LABEL_BG);
    draw_rect_outline(buffer, width, height, x, y, chip_w, chip_h, accent, 1);

    let mut cursor_x = x + LABEL_PAD_X;
    for ch in text.chars() {
        draw_glyph(
            buffer,
            width,
            height,
            cursor_x,
            y + LABEL_PAD_Y,
            ch,
            FONT_SCALE,
            text_color,
        );
        cursor_x += glyph_w;
    }
}

fn draw_filled_rect(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_w: i32,
    rect_h: i32,
    color: [u8; 4],
) {
    for yy in y.max(0)..(y + rect_h).min(height) {
        for xx in x.max(0)..(x + rect_w).min(width) {
            set_pixel(buffer, width, height, xx, yy, color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rect_outline(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    rect_w: i32,
    rect_h: i32,
    color: [u8; 4],
    thickness: i32,
) {
    for t in 0..thickness {
        draw_line(buffer, width, height, x, y + t, x + rect_w, y + t, color, 1);
        draw_line(
            buffer,
            width,
            height,
            x,
            y + rect_h - t,
            x + rect_w,
            y + rect_h - t,
            color,
            1,
        );
        draw_line(buffer, width, height, x + t, y, x + t, y + rect_h, color, 1);
        draw_line(
            buffer,
            width,
            height,
            x + rect_w - t,
            y,
            x + rect_w - t,
            y + rect_h,
            color,
            1,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    thickness: i32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        for ty in -(thickness / 2)..=(thickness / 2) {
            for tx in -(thickness / 2)..=(thickness / 2) {
                set_pixel(buffer, width, height, x0 + tx, y0 + ty, color);
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_filled_circle(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: [u8; 4],
) {
    for y in -radius..=radius {
        for x in -radius..=radius {
            if (x * x) + (y * y) <= radius * radius {
                set_pixel(buffer, width, height, cx + x, cy + y, color);
            }
        }
    }
}

fn draw_circle_outline(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: [u8; 4],
    thickness: i32,
) {
    let outer = radius * radius;
    let inner = (radius - thickness).max(0);
    let inner_sq = inner * inner;
    for y in -radius..=radius {
        for x in -radius..=radius {
            let dist = (x * x) + (y * y);
            if dist <= outer && dist >= inner_sq {
                set_pixel(buffer, width, height, cx + x, cy + y, color);
            }
        }
    }
}

fn set_pixel(buffer: &mut [u8], width: i32, height: i32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= buffer.len() {
        return;
    }
    blend_pixel(&mut buffer[idx..idx + 4], color);
}

fn blend_pixel(pixel: &mut [u8], color: [u8; 4]) {
    let alpha = color[3] as f32 / 255.0;
    let inv = 1.0 - alpha;
    pixel[0] = (color[0] as f32 * alpha + pixel[0] as f32 * inv).round() as u8;
    pixel[1] = (color[1] as f32 * alpha + pixel[1] as f32 * inv).round() as u8;
    pixel[2] = (color[2] as f32 * alpha + pixel[2] as f32 * inv).round() as u8;
    pixel[3] = 255;
}

fn draw_glyph(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    glyph: char,
    scale: i32,
    color: [u8; 4],
) {
    if let Some(rows) = glyph_rows(glyph) {
        for (row_idx, row) in rows.iter().enumerate() {
            for col in 0..5 {
                if row & (1 << (4 - col)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            set_pixel(
                                buffer,
                                width,
                                height,
                                x + (col * scale) + sx,
                                y + (row_idx as i32 * scale) + sy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn glyph_rows(glyph: char) -> Option<[u8; 7]> {
    match glyph.to_ascii_uppercase() {
        '0' => Some([0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E]),
        '1' => Some([0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E]),
        '2' => Some([0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F]),
        '3' => Some([0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E]),
        '4' => Some([0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02]),
        '5' => Some([0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E]),
        '6' => Some([0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E]),
        '7' => Some([0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
        '8' => Some([0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E]),
        '9' => Some([0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C]),
        'A' => Some([0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        'B' => Some([0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E]),
        'C' => Some([0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
        'D' => Some([0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C]),
        'E' => Some([0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
        'F' => Some([0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10]),
        'G' => Some([0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0E]),
        'H' => Some([0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        'I' => Some([0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
        'J' => Some([0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0C]),
        'K' => Some([0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
        'L' => Some([0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
        'M' => Some([0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
        'N' => Some([0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
        'O' => Some([0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        'P' => Some([0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
        'Q' => Some([0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D]),
        'R' => Some([0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
        'S' => Some([0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
        'T' => Some([0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        'U' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        'V' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04]),
        'W' => Some([0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A]),
        'X' => Some([0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
        'Y' => Some([0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04]),
        'Z' => Some([0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F]),
        _ => None,
    }
}

fn region_label_for_language(x: f32, y: f32, language: &str) -> &'static str {
    let vertical = match y {
        y if y < 1.0 / 3.0 => "upper",
        y if y < 2.0 / 3.0 => "center",
        _ => "lower",
    };
    let horizontal = match x {
        x if x < 1.0 / 3.0 => "left",
        x if x < 2.0 / 3.0 => "center",
        _ => "right",
    };

    match language_prefix(language) {
        "ja" => match (vertical, horizontal) {
            ("center", "center") => "中央",
            ("center", "left") => "中央左",
            ("center", "right") => "中央右",
            ("upper", "center") => "上中央",
            ("lower", "center") => "下中央",
            ("upper", "left") => "左上",
            ("upper", "right") => "右上",
            ("lower", "left") => "左下",
            ("lower", "right") => "右下",
            _ => "中央",
        },
        "zh" => match (vertical, horizontal) {
            ("center", "center") => "中央",
            ("center", "left") => "中左",
            ("center", "right") => "中右",
            ("upper", "center") => "上中",
            ("lower", "center") => "下中",
            ("upper", "left") => "左上",
            ("upper", "right") => "右上",
            ("lower", "left") => "左下",
            ("lower", "right") => "右下",
            _ => "中央",
        },
        "ko" => match (vertical, horizontal) {
            ("center", "center") => "중앙",
            ("center", "left") => "중앙 왼쪽",
            ("center", "right") => "중앙 오른쪽",
            ("upper", "center") => "상단 중앙",
            ("lower", "center") => "하단 중앙",
            ("upper", "left") => "왼쪽 상단",
            ("upper", "right") => "오른쪽 상단",
            ("lower", "left") => "왼쪽 하단",
            ("lower", "right") => "오른쪽 하단",
            _ => "중앙",
        },
        "de" => match (vertical, horizontal) {
            ("center", "center") => "Mitte",
            ("center", "left") => "mittig links",
            ("center", "right") => "mittig rechts",
            ("upper", "center") => "oben mittig",
            ("lower", "center") => "unten mittig",
            ("upper", "left") => "oben links",
            ("upper", "right") => "oben rechts",
            ("lower", "left") => "unten links",
            ("lower", "right") => "unten rechts",
            _ => "Mitte",
        },
        "es" => match (vertical, horizontal) {
            ("center", "center") => "centro",
            ("center", "left") => "centro-izquierda",
            ("center", "right") => "centro-derecha",
            ("upper", "center") => "parte superior central",
            ("lower", "center") => "parte inferior central",
            ("upper", "left") => "superior izquierda",
            ("upper", "right") => "superior derecha",
            ("lower", "left") => "inferior izquierda",
            ("lower", "right") => "inferior derecha",
            _ => "centro",
        },
        _ => match (vertical, horizontal) {
            ("center", "center") => "center",
            ("center", other) => match other {
                "left" => "center-left",
                "right" => "center-right",
                _ => "center",
            },
            (other, "center") => match other {
                "upper" => "upper-center",
                "lower" => "lower-center",
                _ => "center",
            },
            (other_v, other_h) => match (other_v, other_h) {
                ("upper", "left") => "upper-left",
                ("upper", "right") => "upper-right",
                ("lower", "left") => "lower-left",
                ("lower", "right") => "lower-right",
                _ => "center",
            },
        },
    }
}

fn language_prefix(language: &str) -> &str {
    language.split(['-', '_']).next().unwrap_or("en")
}

fn prompt_intro(language: &str) -> &'static str {
    match language_prefix(language) {
        "ja" => "添付したスクリーンショットでは、次のマーカーに注目してください:",
        "zh" => "请重点查看附带截图中的以下标记区域：",
        "ko" => "첨부된 스크린샷에서 다음 표시 영역을 중점적으로 봐 주세요:",
        "de" => "Bitte konzentriere dich im angehängten Screenshot auf die folgenden Markierungen:",
        "es" => "Concéntrate en las siguientes marcas de la captura adjunta:",
        _ => "Focus on the marked regions in the attached screenshot:",
    }
}

fn default_pin_note(language: &str) -> &'static str {
    match language_prefix(language) {
        "ja" => "このポイントを重点的に確認してください",
        "zh" => "请重点查看这个点",
        "ko" => "이 지점을 자세히 확인해 주세요",
        "de" => "prüfe diesen Punkt genau",
        "es" => "revisa este punto con detalle",
        _ => "inspect this point closely",
    }
}

fn default_rect_note(language: &str) -> &'static str {
    match language_prefix(language) {
        "ja" => "この範囲の内容を確認してください",
        "zh" => "请查看这个区域中的内容",
        "ko" => "이 영역의 내용을 확인해 주세요",
        "de" => "prüfe den Inhalt in diesem Bereich",
        "es" => "revisa el contenido dentro de esta región",
        _ => "review the contents inside this region",
    }
}

fn format_pin_line(
    language: &str,
    id: &str,
    area: &str,
    pct_x: i32,
    pct_y: i32,
    note: &str,
) -> String {
    match language_prefix(language) {
        "ja" => format!(
            "- Pin {} は {} にあります（左から {}%、上から {}%）。{}。",
            id, area, pct_x, pct_y, note
        ),
        "zh" => format!(
            "- Pin {} 位于{}（距左侧 {}%，距顶部 {}%）：{}。",
            id, area, pct_x, pct_y, note
        ),
        "ko" => format!(
            "- Pin {} 는 {}에 있습니다 (왼쪽에서 {}%, 위에서 {}%). {}.",
            id, area, pct_x, pct_y, note
        ),
        "de" => format!(
            "- Pin {} befindet sich bei {} ({}% von links, {}% von oben): {}.",
            id, area, pct_x, pct_y, note
        ),
        "es" => format!(
            "- El pin {} está en {} ({}% desde la izquierda y {}% desde arriba): {}.",
            id, area, pct_x, pct_y, note
        ),
        _ => format!(
            "- Pin {} at {} ({}% from the left, {}% from the top): {}.",
            id, area, pct_x, pct_y, note
        ),
    }
}

fn format_rect_line(
    language: &str,
    id: &str,
    area: &str,
    x1: i32,
    x2: i32,
    y1: i32,
    y2: i32,
    note: &str,
) -> String {
    match language_prefix(language) {
        "ja" => format!(
            "- Rectangle {} は {} 周辺で、範囲は x={}%-{}%、y={}%-{}% です。{}。",
            id, area, x1, x2, y1, y2, note
        ),
        "zh" => format!(
            "- Rectangle {} 位于{}附近，范围为 x={}%-{}%、y={}%-{}%：{}。",
            id, area, x1, x2, y1, y2, note
        ),
        "ko" => format!(
            "- Rectangle {} 는 {} 주변이며 범위는 x={}%-{}%, y={}%-{}% 입니다. {}.",
            id, area, x1, x2, y1, y2, note
        ),
        "de" => format!(
            "- Rechteck {} liegt bei {} und deckt x={}%-{}%, y={}%-{}% ab: {}.",
            id, area, x1, x2, y1, y2, note
        ),
        "es" => format!(
            "- El rectángulo {} está alrededor de {} y cubre x={}%-{}%, y={}%-{}%: {}.",
            id, area, x1, x2, y1, y2, note
        ),
        _ => format!(
            "- Rectangle {} around {} spanning x={}%-{}%, y={}%-{}%: {}.",
            id, area, x1, x2, y1, y2, note
        ),
    }
}

fn sanitize_note(note: Option<String>) -> Option<String> {
    note.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn alpha_id(index: usize) -> String {
    let mut value = index;
    let mut out = String::new();

    loop {
        let rem = value % 26;
        out.insert(0, (b'A' + rem as u8) as char);
        if value < 26 {
            break;
        }
        value = (value / 26) - 1;
    }

    out
}

fn clamp_unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_hint_contains_positions_and_notes() {
        let annotations = vec![
            Annotation::pin(0, 0.67, 0.48, Some("focus on the broken button".into())),
            Annotation::rectangle(0, 0.1, 0.2, 0.25, 0.3, None),
        ];

        let prompt = build_llm_prompt_hint(&annotations).unwrap();
        assert!(prompt.contains("Pin 1"));
        assert!(prompt.contains("center-right"));
        assert!(prompt.contains("Rectangle A"));
        assert!(prompt.contains("focus on the broken button"));
    }

    #[test]
    fn render_annotations_changes_pixels() {
        let width = 64;
        let height = 48;
        let rgba = vec![240u8; (width * height * 4) as usize];
        let annotations = vec![Annotation::pin(0, 0.5, 0.5, None)];

        let rendered = render_annotations_to_png(&rgba, width, height, &annotations).unwrap();
        assert_ne!(rendered.rgba, rgba);
        assert!(!rendered.encoded_png.is_empty());
    }

    #[test]
    fn alpha_ids_roll_over_after_z() {
        assert_eq!(alpha_id(0), "A");
        assert_eq!(alpha_id(25), "Z");
        assert_eq!(alpha_id(26), "AA");
        assert_eq!(alpha_id(27), "AB");
    }
}
