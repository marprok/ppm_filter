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
    r: f32,
    g: f32,
    b: f32,
}

struct PpmFile {
    width: usize,
    height: usize,
    max_val: usize,
    pixels: Vec<Pixel>,
}

fn parse_ppm(file: &str) -> Result<PpmFile, String> {
    let bytes: Vec<u8> =
        fs::read(file).unwrap_or_else(|error| panic!("Could not read file: {}", error));

    if bytes.len() < 2 {
        return Err(format!("PPM file too small!"));
    }

    let mut from = 0;
    let delims: Vec<u8> = vec![0x20, 0x09, 0x0D, 0x0A, 0x23];

    let magic_number = next_token(&bytes, &mut from, &delims)
        .unwrap_or_else(|error| panic!("Magic number: {}", error));

    let width = next_token(&bytes, &mut from, &delims)
        .unwrap_or_else(|error| panic!("Could not read width: {}", error))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("Width not a number: {}", error));

    let height = next_token(&bytes, &mut from, &delims)
        .unwrap_or_else(|error| panic!("Could not read height: {}", error))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("Height not a number: {}", error));

    let max_color_val = next_token(&bytes, &mut from, &delims)
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
    if bytes[from] == 0x23 || !delims.contains(&bytes[from]) {
        panic!(
            "The header should end with a whitespace but {} found!",
            bytes[from]
        );
    }

    from += 1;
    let mut pixels = Vec::new();
    pixels.reserve(width * height);
    for i in 0..(width * height) {
        pixels.push(Pixel {
            r: bytes[from + i * 3] as f32 / max_color_val as f32,
            g: bytes[from + i * 3 + 1] as f32 / max_color_val as f32,
            b: bytes[from + i * 3 + 2] as f32 / max_color_val as f32,
        });
    }

    Ok(PpmFile {
        width: width,
        height: height,
        max_val: max_color_val,
        pixels: pixels,
    })
}

fn save_ppm(image: &PpmFile, name: &str) -> std::io::Result<()> {
    let mut file = File::create(name)?;
    file.write_all(
        format!("P6\n{}\n{}\n{}\n", image.width, image.height, image.max_val).as_bytes(),
    )?;

    let mut bytes: Vec<u8> = Vec::new();
    bytes.resize(image.pixels.len() * 3, 0u8);
    for (i, pixel) in image.pixels.iter().enumerate() {
        bytes[i * 3] = (pixel.r * 255.0) as u8;
        bytes[i * 3 + 1] = (pixel.g * 255.0) as u8;
        bytes[i * 3 + 2] = (pixel.b * 255.0) as u8;
    }
    file.write_all(&bytes)?;
    Ok(())
}

fn apply_grayscale(image: &mut PpmFile) {
    for pixel in &mut image.pixels {
        pixel.r = pixel.r * 0.216 + pixel.g * 0.7125 + pixel.b * 0.0722;
        pixel.g = pixel.r;
        pixel.b = pixel.r;
    }
}

// 3*3 kernel
fn apply_gaussian_blur(image: &mut PpmFile) {
    let pixels = image.pixels.clone();
    for y in 0..image.height {
        for x in 0..image.width {
            let mut val: f32 = 0.0;
            // previous row
            if y >= 1 {
                if x >= 1 {
                    val += pixels[(y - 1) * image.width + x - 1].r / 16.0;
                }
                val += pixels[(y - 1) * image.width + x].r / 8.0;
                if x + 1 < image.width {
                    val += pixels[(y - 1) * image.width + x + 1].r / 16.0;
                }
            }
            // current row
            if x >= 1 {
                val -= pixels[y * image.width + x - 1].r / 8.0;
            }
            val += pixels[y * image.width + x].r / 4.0;
            if x + 1 < image.width {
                val += pixels[y * image.width + x + 1].r / 8.0;
            }
            // next row
            if y + 1 < image.height {
                if x >= 1 {
                    val += pixels[(y + 1) * image.width + x - 1].r / 16.0;
                }
                val += pixels[(y + 1) * image.width + x].r / 8.0;
                if x + 1 < image.width {
                    val += pixels[(y + 1) * image.width + x + 1].r / 16.0;
                }
            }
            image.pixels[y * image.width + x].r = val;
            image.pixels[y * image.width + x].g = val;
            image.pixels[y * image.width + x].b = val;
        }
    }
}

fn apply_sobel(image: &mut PpmFile) {
    let pixels = image.pixels.clone();
    for y in 0..image.height {
        for x in 0..image.width {
            let mut valx: f32 = 0.0;
            let mut valy: f32 = 0.0;
            // previous row
            if y >= 1 {
                if x >= 1 {
                    valx -= pixels[(y - 1) * image.width + x - 1].r;
                    valy += pixels[(y - 1) * image.width + x - 1].r;
                }
                valy += 2.0 * pixels[(y - 1) * image.width + x].r;
                if x + 1 < image.width {
                    valx += pixels[(y - 1) * image.width + x + 1].r;
                    valy += pixels[(y - 1) * image.width + x + 1].r;
                }
            }
            // current row
            if x >= 1 {
                valx -= 2.0 * pixels[y * image.width + x - 1].r;
            }

            if x + 1 < image.width {
                valx += 2.0 * pixels[y * image.width + x + 1].r;
            }
            // next row
            if y + 1 < image.height {
                if x >= 1 {
                    valx -= pixels[(y + 1) * image.width + x - 1].r;
                    valy -= pixels[(y + 1) * image.width + x - 1].r;
                }
                valy -= 2.0 * pixels[(y + 1) * image.width + x].r;
                if x + 1 < image.width {
                    valx += pixels[(y + 1) * image.width + x + 1].r;
                    valy -= pixels[(y + 1) * image.width + x + 1].r;
                }
            }

            let grad = f32::sqrt(valx * valx + valy * valy);
            if grad > 1.0 {
                image.pixels[y * image.width + x].r = 1.0;
                image.pixels[y * image.width + x].g = 1.0;
                image.pixels[y * image.width + x].b = 1.0;
            } else {
                image.pixels[y * image.width + x].r = grad;
                image.pixels[y * image.width + x].g = grad;
                image.pixels[y * image.width + x].b = grad;
            }
        }
    }
}

#[derive(Clone)]
struct Energy {
    value: u32,
    parent_x: usize,
    parent_y: usize,
    rgb: Pixel,
    intensity: u32,
}

fn resize_width(image: &mut PpmFile, columns: usize) {
    let mut image_energy: Vec<Vec<Energy>> = Vec::new();
    image_energy.reserve(image.height);
    for y in 0..image.height {
        image_energy.push(Vec::new());
        image_energy[y].reserve(image.width);
        for x in 0..image.width {
            image_energy[y].push(Energy {
                value: 0,
                parent_x: x,
                parent_y: y,
                rgb: image.pixels[y * image.width + x],
                intensity: 0,
            });
        }
    }
    apply_grayscale(image);
    apply_gaussian_blur(image);
    apply_sobel(image);
    // store the intensity
    for y in 0..image.height {
        for x in 0..image.width {
            image_energy[y][x].intensity = (image.pixels[y * image.width + x].r * 255.0) as u32;
        }
    }
    image.pixels.clear();
    for _ in 0..columns {
        for y in 0..image.height {
            for x in 0..image.width {
                image_energy[y][x].value = image_energy[y][x].intensity;
                image_energy[y][x].parent_x = x;
                image_energy[y][x].parent_y = y;
            }
        }
        for y in 1..image.height {
            for x in 0..image.width {
                let top_left = if x > 0 {
                    image_energy[y - 1][x - 1].value
                } else {
                    u32::MAX
                };

                let top_center = image_energy[y - 1][x].value;
                let top_right = if x < image.width - 1 {
                    image_energy[y - 1][x + 1].value
                } else {
                    u32::MAX
                };

                if top_left < top_right {
                    if top_left < top_center {
                        image_energy[y][x].value += top_left;
                        if x > 0 {
                            image_energy[y][x].parent_x = x - 1;
                            image_energy[y][x].parent_y = y - 1;
                        }
                    } else {
                        image_energy[y][x].value += top_center;
                        image_energy[y][x].parent_x = x;
                        image_energy[y][x].parent_y = y - 1;
                    }
                } else if top_right < top_center {
                    image_energy[y][x].value += top_right;
                    image_energy[y][x].parent_x = x + 1;
                    image_energy[y][x].parent_y = y - 1;
                } else {
                    image_energy[y][x].value += top_center;
                    image_energy[y][x].parent_x = x;
                    image_energy[y][x].parent_y = y - 1;
                }
            }
        }

        let mut min_id = 0;
        let mut min_energy: u32 = u32::MAX;
        for i in 0..image.width {
            if min_energy > image_energy[image.height - 1][i].value {
                min_energy = image_energy[image.height - 1][i].value;
                min_id = i;
            }
        }
        let mut current_x = min_id;
        let mut current_y = image.height - 1;
        for _ in 0..image.height {
            let parent_x = image_energy[current_y][current_x].parent_x;
            let parent_y = image_energy[current_y][current_x].parent_y;
            image_energy[current_y].remove(current_x);
            if image_energy[current_y].is_empty() {
                image_energy.remove(current_y);
            }
            current_y = parent_y;
            current_x = parent_x;
        }
        image.width -= 1;
    }
    for y in 0..image.height {
        for x in 0..image.width {
            image.pixels.push(image_energy[y][x].rgb);
        }
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
