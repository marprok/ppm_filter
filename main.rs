use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::string::FromUtf8Error;

fn next_token(
    bytes: &Vec<u8>,
    offset: &mut usize,
    delims: &Vec<u8>,
) -> Result<String, FromUtf8Error> {
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
    String::from_utf8(bytes[from..*offset].to_vec())
}

#[derive(Copy, Clone)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}

struct PpmFile {
    max_val: usize,
    pixels: Vec<Vec<Pixel>>,
    w: usize,
    h: usize,
}

impl PpmFile {
    fn to_gray(&self) -> BWImage {
        let mut bw_rows = Vec::new();
        bw_rows.reserve(self.pixels.len());
        for (i, row) in &mut self.pixels.iter().enumerate() {
            bw_rows.push(Vec::new());
            for pixel in row {
                bw_rows[i].push(
                    pixel.r as f32 / self.max_val as f32 * 0.216
                        + pixel.g as f32 / self.max_val as f32 * 0.7125
                        + pixel.b as f32 / self.max_val as f32 * 0.0722,
                );
            }
        }
        BWImage {
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

#[derive(Clone)]
struct BWImage {
    pixels: Vec<Vec<f32>>,
    w: usize,
    h: usize,
}

impl<'a> BWImage {
    fn to_energy(&'a mut self) -> Vec<Vec<Energy>> {
        let mut ret = Vec::new();
        ret.reserve(self.pixels.len());
        for (r_id, row) in self.pixels.iter().enumerate() {
            ret.push(Vec::new());
            ret[r_id].reserve(row.len());
            for (c_id, pixel) in row.iter().enumerate() {
                ret[r_id].push(Energy {
                    value: (pixel * 250.0) as u32,
                    parent_x: c_id,
                    parent_y: r_id,
                })
            }
        }
        ret
    }
    // 3*3 kernel
    fn gaussian_blur(&'a mut self) -> &'a mut BWImage {
        let pixels = self.pixels.clone();
        for y in 0..self.h {
            for x in 0..self.h {
                let mut val: f32 = 0.0;
                // previous row
                if y >= 1 {
                    if x >= 1 {
                        val += pixels[y - 1][x - 1] / 16.0;
                    }
                    val += pixels[y - 1][x] / 8.0;
                    if x + 1 < self.w {
                        val += pixels[y - 1][x + 1] / 16.0;
                    }
                }
                // current row
                if x >= 1 {
                    val += pixels[y][x - 1] / 8.0;
                }
                val += pixels[y][x] / 4.0;
                if x + 1 < self.w {
                    val += pixels[y][x + 1] / 8.0;
                }
                // next row
                if y + 1 < self.h {
                    if x >= 1 {
                        val += pixels[y + 1][x - 1] / 16.0;
                    }
                    val += pixels[y + 1][x] / 8.0;
                    if x + 1 < self.w {
                        val += pixels[y + 1][x + 1] / 16.0;
                    }
                }
                self.pixels[y][x] = val;
                self.pixels[y][x] = val;
                self.pixels[y][x] = val;
            }
        }
        self
    }

    fn sobel(&'a mut self) -> &'a mut BWImage {
        let pixels = self.pixels.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let mut valx: f32 = 0.0;
                let mut valy: f32 = 0.0;
                // previous row
                if y >= 1 {
                    if x >= 1 {
                        valx -= pixels[y - 1][x - 1];
                        valy += pixels[y - 1][x - 1];
                    }
                    valy += 2.0 * pixels[y - 1][x];
                    if x + 1 < self.w {
                        valx += pixels[y - 1][x + 1];
                        valy += pixels[y - 1][x + 1];
                    }
                }
                // current row
                if x >= 1 {
                    valx -= 2.0 * pixels[y][x - 1];
                }

                if x + 1 < self.w {
                    valx += 2.0 * pixels[y][x + 1];
                }
                // next row
                if y + 1 < self.h {
                    if x >= 1 {
                        valx -= pixels[y + 1][x - 1];
                        valy -= pixels[y + 1][x - 1];
                    }
                    valy -= 2.0 * pixels[y + 1][x];
                    if x + 1 < self.w {
                        valx += pixels[y + 1][x + 1];
                        valy -= pixels[y + 1][x + 1];
                    }
                }
                let grad = f32::sqrt(valx * valx + valy * valy);
                self.pixels[y][x] = if grad > 1.0 { 1.0 } else { grad };
            }
        }
        self
    }

    fn to_rgb(&self) -> PpmFile {
        let mut rgb = Vec::new();
        rgb.reserve(self.h);
        for (i, row) in self.pixels.iter().enumerate() {
            rgb.push(Vec::new());
            for pixel in row {
                rgb[i].push(Pixel {
                    r: (pixel * 255.0) as u8,
                    g: (pixel * 255.0) as u8,
                    b: (pixel * 255.0) as u8,
                });
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

fn parse_ppm(file: &str) -> Result<PpmFile, String> {
    let bytes: Vec<u8> =
        fs::read(file).unwrap_or_else(|error| panic!("Could not read file: {}", error));

    if bytes.len() < 2 {
        return Err(format!("PPM file too small!"));
    }

    let mut byte_id = 0;
    let delims: Vec<u8> = vec![0x20, 0x09, 0x0D, 0x0A, 0x23];

    let magic_number = next_token(&bytes, &mut byte_id, &delims)
        .unwrap_or_else(|error| panic!("Magic number: {}", error));

    let width = next_token(&bytes, &mut byte_id, &delims)
        .unwrap_or_else(|error| panic!("Could not read width: {}", error))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("Width not a number: {}", error));

    let height = next_token(&bytes, &mut byte_id, &delims)
        .unwrap_or_else(|error| panic!("Could not read height: {}", error))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("Height not a number: {}", error));

    let max_color_val = next_token(&bytes, &mut byte_id, &delims)
        .unwrap_or_else(|error| panic!("Could not read max color value: {}", error))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("Max color value not a number: {}", error));

    if magic_number != "P6" {
        panic!("Unknown magic number: {}", magic_number);
    }

    if max_color_val != 255 {
        panic!("Maximum color value is not 255!");
    }

    // The last char should be whitespace
    if bytes[byte_id] == 0x23 || !delims.contains(&bytes[byte_id]) {
        panic!(
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

fn save_ppm(image: &PpmFile, name: &str) -> std::io::Result<()> {
    let mut file = File::create(name)?;
    file.write_all(format!("P6\n{}\n{}\n{}\n", image.w, image.h, image.max_val).as_bytes())?;

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

fn resize_width(image: &mut PpmFile, columns: usize) {
    let mut bw = image.to_gray();
    //_ = save_ppm(&bw.to_rgb(), "grey.ppm");
    let bw = bw.gaussian_blur();
    //_ = save_ppm(&bw.to_rgb(), "gaus.ppm");
    let bw = bw.sobel();
    //_ = save_ppm(&bw.to_rgb(), "sobel.ppm");
    for _ in 0..columns {
        let mut energy = bw.to_energy();
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
            /*println!(
                "current {}, {} parent {}, {}",
                current_x, current_y, parent_x, parent_y
            );*/
            bw.pixels[current_y].remove(current_x);
            image.pixels[current_y].remove(current_x);
            if bw.pixels[current_y].is_empty() {
                bw.pixels.remove(current_y);
                image.pixels.remove(current_y);
            }
            current_y = parent_y;
            current_x = parent_x;
        }
        image.w -= 1;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Expected a file and a column number!");
    }

    let mut ppm = parse_ppm(&args[1]).unwrap_or_else(|error| panic!("{}", error));
    let columns_to_remove = args[2]
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("olumns are not a number: {}", error));
    resize_width(&mut ppm, columns_to_remove);

    let out = Path::new(&args[1]);
    save_ppm(
        &ppm,
        &format!(
            "{}_new.ppm",
            out.file_stem()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap()
        ),
    )?;

    Ok(())
}
