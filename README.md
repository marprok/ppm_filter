# Disclaimer
This is my first attempt on writing an actual "useful" program in Rust.
I am sure many things can be done better, I just don't know how at the moment :P
## Description
This tool parses a P6 PPM image file and allows the user to specify any combination of operations to be applied to it.
The supported operations are the following:
- Gaussian Blur
- Sobel Operator
- Grayscale

## Compile
`rustc main.rs`

## Example usage
`./main <file-name> gray gauss sobel`

