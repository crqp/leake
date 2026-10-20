# leake

Converts images, animated GIFs, and videos into texture files for Source Engine
sprays. It writes a Valve Texture Format (`.vtf`) file and, by default, a
matching Valve Material Type (`.vmt`) file.

This is a reimplementation of
[Mishcatt/VTF-Editor](https://github.com/Mishcatt/VTF-Editor), and thus inherits
its behaviour to a fault. Please use
[VTFEdit](https://valvedev.info/tools/vtfedit/) for more advanced creation.

## External Requirements

- `ffmpeg` and `ffprobe`

Still images and GIFs do not need FFmpeg. Supported image formats include PNG,
JPEG, GIF, BMP, TIFF, TGA, WebP, DDS, EXR, Farbfeld, HDR, ICO, PNM, and QOI.

## Usage

Convert one image using automatic sizing:

```sh
leake image.png
```

This writes `spray.vtf` and `spray.vmt` in the current directory.

To choose an output name and directory:

```sh
leake --output image --output-dir output image.png
```

Several still images become animation frames in the order given:

```sh
leake -w 128 frame-01.png frame-02.png frame-03.png
```

An animated GIF or video is treated as one animation. Any additional input paths
are ignored. GIFs are sampled at the spray playback rate by default. Use
`--all-frames` to retain every GIF frame:

```sh
leake --all-frames animation.gif
leake --start 2 --end 8 clip.mp4
```

A single tall still image can be split into equally sized frames:

```sh
leake --strip strip.png
```

Inspect the selected dimensions, frame count, mipmaps, and estimated size
without writing files:

```sh
leake --dry-run -w 256 animation.gif
```

## Options

| Option                      | Description                                                                                             |
| --------------------------- | ------------------------------------------------------------------------------------------------------- |
| `-o, --output NAME`         | Base name for the output files. Default: `spray`.                                                       |
| `-d, --output-dir DIR`      | Directory for output files. Default: current directory.                                                 |
| `-f, --format FORMAT`       | VTF format: `RGBA8888`, `RGB888`, `RGB565`, `BGRA4444`, `BGRA5551`, `DXT1`, or `DXT5`. Default: `DXT1`. |
| `-q, --quality LEVEL`       | DXT compression effort: `FAST`, `NORMAL`, `SLOW`, or `VERY_SLOW`. Default: `SLOW`.                      |
| `-w, --width PIXELS`        | Texture width. If height is omitted, the height uses the same value.                                    |
| `--height PIXELS`           | Texture height. If width is omitted, the width uses the same value.                                     |
| `--mipmaps, --no-mipmaps`   | Enable or disable mipmap levels. Default: enabled where useful.                                         |
| `--mip LEVEL=FILES`         | Replace one mipmap level with one or more images. Repeat the option for more levels.                    |
| `--dither, --no-dither`     | Enable or disable dithering when reducing colours. Default: enabled.                                    |
| `-s, --sampling METHOD`     | Sampling mode: `DEFAULT`, `POINT`, `TRILINEAR`, or `ANISOTROPIC`. Default: `DEFAULT`.                   |
| `--strip`                   | Split one tall still image into animation frames.                                                       |
| `--rescale, --no-rescale`   | Fit each source frame into its output slot. Default: enabled.                                           |
| `--speed FACTOR`            | Playback speed factor for imported animations. Default: `1`.                                            |
| `--start SECONDS`           | First video second or GIF frame to import. Default: `0`.                                                |
| `--end SECONDS`             | Last video second or GIF frame to import.                                                               |
| `--all-frames`              | Keep every GIF frame instead of sampling at spray playback rate.                                        |
| `--vmt, --no-vmt`           | Write or suppress the matching material file. Default: enabled.                                         |
| `-j, --jobs COUNT`          | Number of encoding threads. Default: available processors.                                              |
| `--gpu`                     | Encode with the GPU. Falls back to the CPU when no usable adapter is present.                           |
| `--progress, --no-progress` | Show or suppress encoding progress on stderr. Default: enabled in a terminal.                           |
| `--dry-run`                 | Report the planned layout without writing output files.                                                 |

### Mipmap replacements

Mipmap levels are numbered from `1`, with level `1` being the first level below
the base texture. A replacement can contain several images. They are used in
order and repeated if the animation has more frames than the replacement set:

```sh
leake -w 256 --mip 1=small-a.png,small-b.png source.png
```

### GPU encoding

`--gpu` encodes on the GPU. DXT compression may produce slightly different
bytes from the CPU encoder. `--jobs` only affects the CPU encoder.
