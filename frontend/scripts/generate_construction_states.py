"""First-pass pixel-art construction-state spritesheet for a muted city-sim.

Requirements:
    Python 3
    Pillow: pip install pillow

Output:
    construction-states.png
    Image size: 96x48 pixels
    Grid: 2 columns x 1 row
    Frame size: 48x48 pixels

Frame order:
    0 empty lot
    1 under construction

The sprites use a transparent background and crisp integer-aligned drawing.
No anti-aliasing, resizing, blur, or smoothing is applied.
"""

from pathlib import Path
from PIL import Image, ImageDraw

FRAME_SIZE = 48
COLUMNS = 2
ROWS = 1
OUTPUT_FILE = Path(__file__).resolve().parents[1] / "public" / "sprites" / "construction-states.png"

PALETTE = {
    "outline": "#252C2D",
    "deep_shadow": "#303738",
    "ground_shadow": "#263032",
    "dirt": "#655544",
    "dirt_dark": "#514536",
    "dirt_mid": "#75624D",
    "dirt_light": "#84715A",
    "wood": "#80694F",
    "wood_light": "#9B8060",
    "wood_dark": "#5F4E3D",
    "metal": "#707778",
    "metal_light": "#8A9190",
    "metal_dark": "#50595A",
    "wall_warm": "#967E65",
    "wall_warm_light": "#AB9276",
    "wall_warm_dark": "#796550",
    "roof_warm": "#765347",
    "roof_warm_light": "#8D6553",
    "roof_warm_dark": "#593E39",
    "sign_bg": "#665748",
    "sign_light": "#A28C68",
    "trim": "#D0BD93",
    "grass": "#42563F",
    "grass_light": "#58704F",
    "grass_dark": "#344832",
}


def rect(draw: ImageDraw.ImageDraw, box: tuple[int, int, int, int], color: str) -> None:
    draw.rectangle(box, fill=color)


def pixel(draw: ImageDraw.ImageDraw, x: int, y: int, color: str) -> None:
    draw.point((x, y), fill=color)


def line(draw: ImageDraw.ImageDraw, points: tuple[int, int, int, int], color: str, width: int = 1) -> None:
    draw.line(points, fill=color, width=width)


def polygon(draw: ImageDraw.ImageDraw, points: list[tuple[int, int]], color: str) -> None:
    draw.polygon(points, fill=color)


def make_frame() -> Image.Image:
    return Image.new("RGBA", (FRAME_SIZE, FRAME_SIZE), (0, 0, 0, 0))


def add_ground_shadow(draw: ImageDraw.ImageDraw, x1: int, y1: int, x2: int, y2: int) -> None:
    rect(draw, (x1 + 2, y1, x2 - 2, y2), PALETTE["ground_shadow"])
    rect(draw, (x1, y1 + 1, x2, y2 - 1), PALETTE["ground_shadow"])


def add_grass_tuft(draw: ImageDraw.ImageDraw, x: int, y: int) -> None:
    pixel(draw, x, y, PALETTE["grass"])
    pixel(draw, x - 1, y + 1, PALETTE["grass_dark"])
    pixel(draw, x + 1, y + 1, PALETTE["grass_light"])


def draw_empty_lot() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 6, 39, 42, 44)
    rect(draw, (8, 30, 40, 41), PALETTE["dirt_dark"])
    rect(draw, (9, 29, 39, 40), PALETTE["dirt"])
    rect(draw, (11, 30, 37, 31), PALETTE["dirt_mid"])
    for x, y in ((13, 34), (19, 32), (26, 35), (34, 33), (16, 38), (30, 39), (37, 36)):
      pixel(draw, x, y, PALETTE["dirt_dark"])
    for x, y in ((11, 32), (22, 37), (28, 32), (35, 38)):
      pixel(draw, x, y, PALETTE["dirt_light"])
    for x, y in ((10, 27), (38, 27), (10, 38), (38, 38)):
      rect(draw, (x, y, x + 1, y + 5), PALETTE["wood_dark"])
      pixel(draw, x, y, PALETTE["wood_light"])
    line(draw, (11, 29, 37, 29), PALETTE["wood"])
    line(draw, (11, 39, 37, 39), PALETTE["wood_dark"])
    line(draw, (11, 30, 11, 38), PALETTE["wood_dark"])
    line(draw, (37, 30, 37, 38), PALETTE["wood_dark"])
    rect(draw, (17, 20, 19, 33), PALETTE["wood_dark"])
    rect(draw, (18, 21, 18, 32), PALETTE["wood_light"])
    rect(draw, (14, 20, 26, 27), PALETTE["outline"])
    rect(draw, (15, 21, 25, 26), PALETTE["sign_bg"])
    rect(draw, (17, 22, 23, 22), PALETTE["sign_light"])
    rect(draw, (17, 24, 21, 24), PALETTE["trim"])
    add_grass_tuft(draw, 7, 38)
    add_grass_tuft(draw, 42, 39)
    add_grass_tuft(draw, 34, 28)
    return frame


def draw_under_construction() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 6, 41, 42, 45)
    rect(draw, (8, 37, 40, 42), PALETTE["dirt_dark"])
    rect(draw, (9, 36, 39, 41), PALETTE["dirt"])
    line(draw, (11, 37, 37, 37), PALETTE["dirt_mid"])
    rect(draw, (11, 25, 37, 40), PALETTE["outline"])
    rect(draw, (12, 26, 36, 39), PALETTE["wall_warm_dark"])
    rect(draw, (13, 27, 20, 38), PALETTE["wall_warm"])
    rect(draw, (29, 27, 35, 38), PALETTE["wall_warm"])
    rect(draw, (14, 28, 19, 29), PALETTE["wall_warm_light"])
    rect(draw, (30, 28, 34, 29), PALETTE["wall_warm_light"])
    rect(draw, (22, 30, 27, 39), PALETTE["deep_shadow"])
    line(draw, (22, 30, 27, 30), PALETTE["wood_light"])
    line(draw, (22, 30, 22, 39), PALETTE["wood"])
    line(draw, (27, 30, 27, 39), PALETTE["wood_dark"])
    for x in (12, 20, 28, 36):
      rect(draw, (x, 23, x + 1, 40), PALETTE["wood_dark"])
      pixel(draw, x, 23, PALETTE["wood_light"])
    line(draw, (12, 26, 37, 26), PALETTE["wood"])
    line(draw, (12, 34, 37, 34), PALETTE["wood_dark"])
    polygon(draw, [(8, 25), (20, 14), (28, 14), (40, 25)], PALETTE["outline"])
    line(draw, (10, 24, 21, 15), PALETTE["wood"])
    line(draw, (38, 24, 27, 15), PALETTE["wood"])
    line(draw, (21, 15, 27, 15), PALETTE["wood_light"])
    line(draw, (14, 22, 34, 22), PALETTE["wood_dark"])
    polygon(draw, [(11, 23), (20, 16), (23, 16), (15, 23)], PALETTE["roof_warm"])
    line(draw, (12, 22, 20, 16), PALETTE["roof_warm_light"])
    polygon(draw, [(29, 17), (37, 23), (33, 23), (27, 17)], PALETTE["roof_warm_dark"])
    for x in (7, 41):
      rect(draw, (x, 20, x + 1, 40), PALETTE["metal_dark"])
      pixel(draw, x + 1, 20, PALETTE["metal_light"])
    line(draw, (7, 24, 41, 24), PALETTE["metal"])
    line(draw, (7, 32, 41, 32), PALETTE["metal_dark"])
    line(draw, (8, 21, 14, 32), PALETTE["metal_dark"])
    line(draw, (14, 21, 8, 32), PALETTE["metal"])
    line(draw, (35, 21, 41, 32), PALETTE["metal_dark"])
    line(draw, (41, 21, 35, 32), PALETTE["metal"])
    rect(draw, (30, 39, 38, 41), PALETTE["wood_dark"])
    line(draw, (31, 39, 37, 39), PALETTE["wood_light"])
    rect(draw, (9, 39, 14, 41), PALETTE["metal_dark"])
    pixel(draw, 10, 39, PALETTE["metal_light"])
    return frame


def build_spritesheet() -> Image.Image:
    frames = [draw_empty_lot(), draw_under_construction()]
    sheet = Image.new("RGBA", (COLUMNS * FRAME_SIZE, ROWS * FRAME_SIZE), (0, 0, 0, 0))
    for index, frame in enumerate(frames):
        column = index % COLUMNS
        row = index // COLUMNS
        sheet.paste(frame, (column * FRAME_SIZE, row * FRAME_SIZE), frame)
    return sheet


def main() -> None:
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    spritesheet = build_spritesheet()
    spritesheet.save(OUTPUT_FILE, format="PNG")
    print(f"Saved: {OUTPUT_FILE.resolve()}")


if __name__ == "__main__":
    main()
