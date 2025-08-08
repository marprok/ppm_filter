use anyhow::{bail, Context, Result};
use clap::Parser;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// Resize a PPM image
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to PPM file
    #[arg(short, long)]
    file: PathBuf,

    /// Number of columns to remove
    #[arg(short, long)]
    cols: usize,
}

fn next_token(bytes: &Vec<u8>, offset: &mut usize, delims: &Vec<u8>) -> Result<String> {
    // skip depims and comments
    while delims.contains(&bytes[*offset]) {
        // skip the entire line in case of comments
        if bytes[*offset] == 0x23 {
            *offset += 1;
            while bytes[*offset] != 0x0A {
                *offset += 1;
            }
        }
        *offset += 1;
    }

    let from: usize = *offset;
    for byte in &bytes[from..] {
        if delims.contains(byte) {
            break;
        }
        *offset += 1;
    }
    Ok(String::from_utf8(bytes[from..*offset].to_vec())
        .context(format!("Could not parse bytes at offset {}", offset))?)
}

#[derive(Copy, Clone)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}
#[derive(Clone)]
struct PixelIntensity {
    rgb: Pixel,
    intensity: f32,
}

struct PpmFile<T> {
    max_val: usize,
    pixels: Vec<Vec<T>>,
    w: usize,
    h: usize,
}

impl PpmFile<Pixel> {
    fn to_gray(&self) -> PpmFile<PixelIntensity> {
        let mut bw_rows = Vec::new();
        bw_rows.reserve(self.pixels.len());
        for (i, row) in &mut self.pixels.iter().enumerate() {
            bw_rows.push(Vec::new());
            for pixel in row {
                bw_rows[i].push(PixelIntensity {
                    rgb: *pixel,
                    intensity: pixel.r as f32 / self.max_val as f32 * 0.216
                        + pixel.g as f32 / self.max_val as f32 * 0.7125
                        + pixel.b as f32 / self.max_val as f32 * 0.0722,
                });
            }
        }
        PpmFile {
            max_val: self.max_val,
            pixels: bw_rows,
            w: self.w,
            h: self.h,
        }
    }
}

struct Energy {
    value: u32,
    parent_x: usize,
    parent_y: usize,
}

impl<'a> PpmFile<PixelIntensity> {
    fn to_energy(&'a mut self) -> Vec<Vec<Energy>> {
        let mut ret = Vec::new();
        ret.reserve(self.pixels.len());
        for (r_id, row) in self.pixels.iter().enumerate() {
            ret.push(Vec::new());
            ret[r_id].reserve(row.len());
            for (c_id, pixel) in row.iter().enumerate() {
                ret[r_id].push(Energy {
                    value: (pixel.intensity * 250.0) as u32,
                    parent_x: c_id,
                    parent_y: r_id,
                })
            }
        }
        ret
    }
    // 3*3 kernel
    fn gaussian_blur(&'a mut self) -> &'a mut PpmFile<PixelIntensity> {
        let pixels = self.pixels.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let mut val: f32 = 0.0;
                // previous row
                if y >= 1 {
                    if x >= 1 {
                        val += pixels[y - 1][x - 1].intensity / 16.0;
                    }
                    val += pixels[y - 1][x].intensity / 8.0;
                    if x + 1 < self.w {
                        val += pixels[y - 1][x + 1].intensity / 16.0;
                    }
                }
                // current row
                if x >= 1 {
                    val += pixels[y][x - 1].intensity / 8.0;
                }
                val += pixels[y][x].intensity / 4.0;
                if x + 1 < self.w {
                    val += pixels[y][x + 1].intensity / 8.0;
                }
                // next row
                if y + 1 < self.h {
                    if x >= 1 {
                        val += pixels[y + 1][x - 1].intensity / 16.0;
                    }
                    val += pixels[y + 1][x].intensity / 8.0;
                    if x + 1 < self.w {
                        val += pixels[y + 1][x + 1].intensity / 16.0;
                    }
                }
                self.pixels[y][x].intensity = val;
            }
        }
        self
    }

    fn sobel(&'a mut self) -> &'a mut PpmFile<PixelIntensity> {
        let pixels = self.pixels.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let mut valx: f32 = 0.0;
                let mut valy: f32 = 0.0;
                // previous row
                if y >= 1 {
                    if x >= 1 {
                        valx += pixels[y - 1][x - 1].intensity;
                        valy += pixels[y - 1][x - 1].intensity;
                    }
                    valy += 2.0 * pixels[y - 1][x].intensity;
                    if x + 1 < self.w {
                        valx -= pixels[y - 1][x + 1].intensity;
                        valy += pixels[y - 1][x + 1].intensity;
                    }
                }
                // current row
                if x >= 1 {
                    valx += 2.0 * pixels[y][x - 1].intensity;
                }

                if x + 1 < self.w {
                    valx -= 2.0 * pixels[y][x + 1].intensity;
                }
                // next row
                if y + 1 < self.h {
                    if x >= 1 {
                        valx += pixels[y + 1][x - 1].intensity;
                        valy -= pixels[y + 1][x - 1].intensity;
                    }
                    valy -= 2.0 * pixels[y + 1][x].intensity;
                    if x + 1 < self.w {
                        valx -= pixels[y + 1][x + 1].intensity;
                        valy -= pixels[y + 1][x + 1].intensity;
                    }
                }
                let grad = f32::sqrt(valx * valx + valy * valy);
                self.pixels[y][x].intensity = if grad > 1.0 { 1.0 } else { grad };
            }
        }
        self
    }

    fn to_rgb(&self) -> PpmFile<Pixel> {
        let mut rgb = Vec::new();
        rgb.reserve(self.h);
        for (i, row) in self.pixels.iter().enumerate() {
            rgb.push(Vec::new());
            for pixel in row {
                rgb[i].push(pixel.rgb);
            }
        }
        PpmFile {
            max_val: 255,
            pixels: rgb,
            w: self.w,
            h: self.h,
        }
    }
}

fn parse_ppm(file: &str) -> Result<PpmFile<Pixel>> {
    let bytes: Vec<u8> = fs::read(file).context(format!("Failed to load file {}", file))?;

    if bytes.len() < 2 {
        bail!(
            "File size is too small to be a valid PPM image! {}",
            bytes.len()
        );
    }

    let mut byte_id = 0;
    let delims: Vec<u8> = vec![0x20, 0x09, 0x0D, 0x0A, 0x23];

    let magic_number =
        next_token(&bytes, &mut byte_id, &delims).context("Failed while reading magic number")?;

    let width = next_token(&bytes, &mut byte_id, &delims)
        .context("Failed while reading width")?
        .parse::<usize>()
        .context("Width not a number")?;

    let height = next_token(&bytes, &mut byte_id, &delims)
        .context("Failed while reading height")?
        .parse::<usize>()
        .context("Height not a number")?;

    let max_color_val = next_token(&bytes, &mut byte_id, &delims)
        .context("Failed while reading max color value")?
        .parse::<usize>()
        .context("Max color value not a number")?;

    if magic_number != "P6" {
        bail!("Unknown magic number: {}", magic_number);
    }

    if max_color_val != 255 {
        bail!("Maximum color value is not 255!");
    }

    // The last char should be whitespace
    if bytes[byte_id] == 0x23 || !delims.contains(&bytes[byte_id]) {
        bail!(
            "The header should end with a whitespace but {} found!",
            bytes[byte_id]
        );
    }

    byte_id += 1;
    let mut pixels = Vec::new();
    pixels.reserve(height);
    for y in 0..(height) {
        pixels.push(Vec::new());
        for x in 0..width {
            pixels[y].push(Pixel {
                r: bytes[byte_id + (y * width + x) * 3],
                g: bytes[byte_id + (y * width + x) * 3 + 1],
                b: bytes[byte_id + (y * width + x) * 3 + 2],
            });
        }
    }

    Ok(PpmFile {
        w: width,
        h: height,
        max_val: max_color_val,
        pixels: pixels,
    })
}

fn save_ppm(image: &PpmFile<Pixel>, name: &str) -> Result<()> {
    let mut file = File::create(name)?;
    file.write_all(format!("P6\n{}\n{}\n{}\n", image.w, image.h, image.max_val).as_bytes())
        .context(format!(
            "Could not write image header P6\n{}\n{}\n{}\n",
            image.w, image.h, image.max_val
        ))?;

    let mut bytes: Vec<u8> = Vec::new();
    bytes.resize(image.w * image.h * 3, 0u8);
    for (row, pixels) in image.pixels.iter().enumerate() {
        for (col, pixel) in pixels.iter().enumerate() {
            bytes[(row * image.w + col) * 3] = pixel.r;
            bytes[(row * image.w + col) * 3 + 1] = pixel.g;
            bytes[(row * image.w + col) * 3 + 2] = pixel.b;
        }
    }
    file.write_all(&bytes)?;
    Ok(())
}

fn resize_width(image: PpmFile<Pixel>, columns: usize) -> PpmFile<Pixel> {
    let mut image = image.to_gray();
    let image = image.gaussian_blur().sobel();
    for _ in 0..columns {
        let mut energy = image.to_energy();
        for y in 1..image.h {
            for x in 0..image.w {
                let top_left = if x > 0 {
                    energy[y - 1][x - 1].value
                } else {
                    u32::MAX
                };

                let top_center = energy[y - 1][x].value;
                let top_right = if x < image.w - 1 {
                    energy[y - 1][x + 1].value
                } else {
                    u32::MAX
                };

                if top_left < top_right {
                    if top_left < top_center {
                        energy[y][x].value += top_left;
                        if x > 0 {
                            energy[y][x].parent_x = x - 1;
                            energy[y][x].parent_y = y - 1;
                        }
                    } else {
                        energy[y][x].value += top_center;
                        energy[y][x].parent_x = x;
                        energy[y][x].parent_y = y - 1;
                    }
                } else if top_right < top_center {
                    energy[y][x].value += top_right;
                    energy[y][x].parent_x = x + 1;
                    energy[y][x].parent_y = y - 1;
                } else {
                    energy[y][x].value += top_center;
                    energy[y][x].parent_x = x;
                    energy[y][x].parent_y = y - 1;
                }
            }
        }

        let mut min_id = 0;
        let mut min_energy: u32 = u32::MAX;
        for (i, pixel) in energy[image.h - 1].iter().enumerate() {
            if min_energy > pixel.value {
                min_energy = pixel.value;
                min_id = i;
            }
        }
        let mut current_x = min_id;
        let mut current_y = image.h - 1;
        for _ in 0..image.h {
            let parent_x = energy[current_y][current_x].parent_x;
            let parent_y = energy[current_y][current_x].parent_y;
            image.pixels[current_y].remove(current_x);
            if image.pixels[current_y].is_empty() {
                image.pixels.remove(current_y);
            }
            current_y = parent_y;
            current_x = parent_x;
        }
        image.w -= 1;
    }
    image.to_rgb()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file_path = args.file.display().to_string();
    let mut ppm = parse_ppm(&file_path).context(format!("Could not parse {}", file_path))?;
    ppm = resize_width(ppm, args.cols);
    save_ppm(
        &ppm,
        &format!("{}_new.ppm", args.file.file_stem().unwrap().display()),
    )?;

    Ok(())
}
