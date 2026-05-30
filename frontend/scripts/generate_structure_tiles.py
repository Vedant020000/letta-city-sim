"""First-pass pixel-art structure spritesheet for a muted, darkish city-sim.

Requirements:
    Python 3
    Pillow: pip install pillow

Output:
    structure-tiles.png
    Image size: 144x96 pixels
    Grid: 3 columns x 2 rows
    Frame size: 48x48 pixels

The spritesheet uses a transparent background and crisp integer-aligned
pixel drawing. No anti-aliasing, resizing, blur, or smoothing is applied.
"""

from pathlib import Path
from PIL import Image, ImageDraw

FRAME_SIZE = 48
COLUMNS = 3
ROWS = 2
OUTPUT_FILE = Path(__file__).resolve().parents[1] / "public" / "sprites" / "structure-tiles.png"

PALETTE = {
    "outline": "#252C2D",
    "deep_shadow": "#303738",
    "ground_shadow": "#263032",
    "wall_warm": "#967E65",
    "wall_warm_light": "#AB9276",
    "wall_warm_dark": "#796550",
    "wall_cool": "#778083",
    "wall_cool_light": "#929B9B",
    "wall_cool_dark": "#5E686B",
    "wall_stone": "#82827A",
    "wall_stone_light": "#98988D",
    "wall_stone_dark": "#656760",
    "roof_warm": "#765347",
    "roof_warm_light": "#8D6553",
    "roof_warm_dark": "#593E39",
    "roof_cool": "#4C5B60",
    "roof_cool_light": "#647278",
    "roof_cool_dark": "#39474B",
    "roof_green": "#50604F",
    "roof_green_dark": "#3D4B3E",
    "door": "#55463A",
    "door_light": "#725E4C",
    "glass": "#66818A",
    "glass_light": "#83A0A5",
    "glass_dark": "#415F68",
    "window_glow": "#B29A68",
    "sign_bg": "#665748",
    "sign_light": "#A28C68",
    "metal": "#707778",
    "metal_light": "#8A9190",
    "metal_dark": "#50595A",
    "trim": "#D0BD93",
    "leaf_dark": "#304735",
    "leaf_mid": "#436047",
    "leaf_light": "#5C7755",
    "leaf_shadow": "#283D2E",
    "trunk": "#5C4836",
    "trunk_light": "#765D45",
    "grass": "#42563F",
    "grass_light": "#58704F",
    "flower": "#A78964",
    "path": "#6A665C",
    "path_light": "#7C776C",
    "path_dark": "#555249",
    "civic_banner": "#6E5D52",
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


def add_window(draw: ImageDraw.ImageDraw, x: int, y: int, width: int = 5, height: int = 6, glow: bool = False) -> None:
    rect(draw, (x, y, x + width - 1, y + height - 1), PALETTE["outline"])
    fill = PALETTE["window_glow"] if glow else PALETTE["glass"]
    rect(draw, (x + 1, y + 1, x + width - 2, y + height - 2), fill)
    if width >= 5:
      line(draw, (x + width // 2, y + 1, x + width // 2, y + height - 2), PALETTE["glass_dark"])
    if height >= 6:
      line(draw, (x + 1, y + height // 2, x + width - 2, y + height // 2), PALETTE["glass_dark"])
    pixel(draw, x + 1, y + 1, PALETTE["glass_light"])


def add_door(draw: ImageDraw.ImageDraw, x: int, y: int, width: int = 6, height: int = 9, glass_panel: bool = False) -> None:
    rect(draw, (x, y, x + width - 1, y + height - 1), PALETTE["outline"])
    rect(draw, (x + 1, y + 1, x + width - 2, y + height - 1), PALETTE["door"])
    if glass_panel:
      rect(draw, (x + 2, y + 2, x + width - 3, y + 5), PALETTE["glass_dark"])
      pixel(draw, x + 2, y + 2, PALETTE["glass_light"])
    pixel(draw, x + width - 2, y + height // 2 + 1, PALETTE["trim"])


def draw_home() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 9, 41, 39, 44)
    rect(draw, (19, 40, 29, 42), PALETTE["path_dark"])
    rect(draw, (20, 39, 28, 40), PALETTE["path"])
    rect(draw, (10, 22, 38, 40), PALETTE["outline"])
    rect(draw, (11, 23, 37, 39), PALETTE["wall_warm"])
    rect(draw, (12, 24, 36, 25), PALETTE["wall_warm_light"])
    rect(draw, (11, 37, 37, 39), PALETTE["wall_warm_dark"])
    polygon(draw, [(7, 23), (20, 12), (28, 12), (41, 23)], PALETTE["outline"])
    polygon(draw, [(9, 22), (21, 13), (27, 13), (39, 22)], PALETTE["roof_warm"])
    line(draw, (11, 21, 37, 21), PALETTE["roof_warm_dark"])
    line(draw, (21, 14, 27, 14), PALETTE["roof_warm_light"])
    rect(draw, (31, 12, 35, 19), PALETTE["outline"])
    rect(draw, (32, 13, 34, 19), PALETTE["roof_warm_dark"])
    rect(draw, (31, 12, 35, 13), PALETTE["roof_warm_light"])
    add_window(draw, 14, 27, width=6, height=7, glow=True)
    add_window(draw, 29, 27, width=6, height=7, glow=True)
    add_door(draw, 22, 30, width=6, height=10)
    rect(draw, (12, 39, 18, 40), PALETTE["wall_warm_dark"])
    rect(draw, (30, 39, 36, 40), PALETTE["wall_warm_dark"])
    return frame


def draw_shop() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 7, 41, 41, 44)
    rect(draw, (16, 40, 32, 42), PALETTE["path_dark"])
    rect(draw, (17, 39, 31, 40), PALETTE["path"])
    rect(draw, (8, 17, 40, 40), PALETTE["outline"])
    rect(draw, (9, 18, 39, 39), PALETTE["wall_warm"])
    rect(draw, (10, 19, 38, 22), PALETTE["wall_warm_light"])
    rect(draw, (9, 36, 39, 39), PALETTE["wall_warm_dark"])
    rect(draw, (7, 14, 41, 18), PALETTE["outline"])
    rect(draw, (8, 15, 40, 17), PALETTE["roof_cool"])
    line(draw, (9, 15, 39, 15), PALETTE["roof_cool_light"])
    rect(draw, (13, 20, 35, 25), PALETTE["outline"])
    rect(draw, (14, 21, 34, 24), PALETTE["sign_bg"])
    rect(draw, (17, 22, 31, 22), PALETTE["sign_light"])
    pixel(draw, 15, 21, PALETTE["trim"])
    pixel(draw, 33, 24, PALETTE["roof_warm_dark"])
    rect(draw, (10, 26, 38, 30), PALETTE["outline"])
    rect(draw, (11, 27, 37, 29), PALETTE["roof_warm"])
    for x in range(12, 38, 6):
      rect(draw, (x, 27, x + 2, 29), PALETTE["trim"])
    for x in (11, 17, 23, 29, 35):
      pixel(draw, x, 30, PALETTE["roof_warm_dark"])
    rect(draw, (11, 31, 20, 38), PALETTE["outline"])
    rect(draw, (12, 32, 19, 37), PALETTE["glass"])
    pixel(draw, 12, 32, PALETTE["glass_light"])
    line(draw, (15, 32, 15, 37), PALETTE["glass_dark"])
    add_door(draw, 22, 30, width=6, height=10, glass_panel=True)
    rect(draw, (29, 31, 37, 38), PALETTE["outline"])
    rect(draw, (30, 32, 36, 37), PALETTE["glass"])
    pixel(draw, 30, 32, PALETTE["glass_light"])
    line(draw, (33, 32, 33, 37), PALETTE["glass_dark"])
    return frame


def draw_civic() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 5, 42, 43, 45)
    rect(draw, (17, 40, 31, 43), PALETTE["path_dark"])
    rect(draw, (18, 39, 30, 41), PALETTE["path"])
    rect(draw, (19, 38, 29, 39), PALETTE["path_light"])
    rect(draw, (8, 21, 40, 40), PALETTE["outline"])
    rect(draw, (9, 22, 39, 39), PALETTE["wall_stone"])
    rect(draw, (10, 23, 38, 24), PALETTE["wall_stone_light"])
    rect(draw, (9, 37, 39, 39), PALETTE["wall_stone_dark"])
    polygon(draw, [(6, 22), (24, 12), (42, 22)], PALETTE["outline"])
    polygon(draw, [(9, 21), (24, 14), (39, 21)], PALETTE["roof_cool"])
    line(draw, (12, 20, 36, 20), PALETTE["roof_cool_light"])
    rect(draw, (21, 14, 27, 20), PALETTE["outline"])
    rect(draw, (22, 15, 26, 19), PALETTE["wall_stone_light"])
    rect(draw, (23, 16, 25, 18), PALETTE["glass_dark"])
    pixel(draw, 24, 16, PALETTE["trim"])
    pixel(draw, 24, 17, PALETTE["trim"])
    for x in (12, 18, 29, 35):
      rect(draw, (x, 25, x + 2, 38), PALETTE["outline"])
      rect(draw, (x + 1, 26, x + 1, 37), PALETTE["wall_stone_light"])
    rect(draw, (21, 28, 27, 39), PALETTE["outline"])
    rect(draw, (22, 29, 26, 38), PALETTE["door"])
    line(draw, (24, 29, 24, 38), PALETTE["door_light"])
    pixel(draw, 25, 34, PALETTE["trim"])
    add_window(draw, 14, 27, width=4, height=6)
    add_window(draw, 30, 27, width=4, height=6)
    rect(draw, (10, 27, 11, 33), PALETTE["civic_banner"])
    rect(draw, (37, 27, 38, 33), PALETTE["civic_banner"])
    return frame


def draw_workplace() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 6, 42, 42, 45)
    rect(draw, (18, 40, 30, 43), PALETTE["path_dark"])
    rect(draw, (19, 39, 29, 41), PALETTE["path"])
    rect(draw, (8, 16, 40, 40), PALETTE["outline"])
    rect(draw, (9, 17, 39, 39), PALETTE["wall_cool"])
    rect(draw, (10, 18, 38, 20), PALETTE["wall_cool_light"])
    rect(draw, (9, 36, 39, 39), PALETTE["wall_cool_dark"])
    rect(draw, (7, 13, 41, 17), PALETTE["outline"])
    rect(draw, (8, 14, 40, 16), PALETTE["roof_cool"])
    line(draw, (9, 14, 39, 14), PALETTE["roof_cool_light"])
    rect(draw, (29, 10, 36, 14), PALETTE["outline"])
    rect(draw, (30, 11, 35, 13), PALETTE["metal_dark"])
    line(draw, (31, 11, 34, 11), PALETTE["metal_light"])
    rect(draw, (15, 19, 33, 23), PALETTE["outline"])
    rect(draw, (16, 20, 32, 22), PALETTE["sign_bg"])
    line(draw, (19, 21, 29, 21), PALETTE["sign_light"])
    for x in (12, 20, 29):
      add_window(draw, x, 25, width=6, height=6)
    rect(draw, (20, 32, 28, 40), PALETTE["outline"])
    rect(draw, (21, 33, 27, 39), PALETTE["glass_dark"])
    line(draw, (24, 33, 24, 39), PALETTE["metal"])
    pixel(draw, 22, 34, PALETTE["glass_light"])
    rect(draw, (10, 38, 19, 39), PALETTE["wall_cool_dark"])
    rect(draw, (29, 38, 38, 39), PALETTE["wall_cool_dark"])
    return frame


def add_tree(draw: ImageDraw.ImageDraw, x: int, y: int, scale: int = 1) -> None:
    rect(draw, (x - scale, y, x + scale, y + 7), PALETTE["trunk"])
    rect(draw, (x, y + 1, x + scale, y + 6), PALETTE["trunk_light"])
    rect(draw, (x - 6 * scale, y - 10 * scale, x + 6 * scale, y + 1 * scale), PALETTE["leaf_shadow"])
    rect(draw, (x - 8 * scale, y - 7 * scale, x + 8 * scale, y - 2 * scale), PALETTE["leaf_shadow"])
    rect(draw, (x - 5 * scale, y - 9 * scale, x + 5 * scale, y), PALETTE["leaf_mid"])
    rect(draw, (x - 7 * scale, y - 6 * scale, x + 7 * scale, y - 3 * scale), PALETTE["leaf_mid"])
    rect(draw, (x - 4 * scale, y - 8 * scale, x - scale, y - 6 * scale), PALETTE["leaf_light"])
    rect(draw, (x + 2 * scale, y - 5 * scale, x + 5 * scale, y - 3 * scale), PALETTE["leaf_dark"])
    pixel(draw, x + 4 * scale, y - 7 * scale, PALETTE["leaf_light"])


def draw_park() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 5, 41, 43, 44)
    rect(draw, (7, 34, 41, 41), PALETTE["leaf_shadow"])
    rect(draw, (8, 33, 40, 40), PALETTE["grass"])
    rect(draw, (9, 34, 39, 35), PALETTE["grass_light"])
    rect(draw, (18, 34, 30, 41), PALETTE["path_dark"])
    rect(draw, (20, 33, 28, 41), PALETTE["path"])
    rect(draw, (21, 33, 27, 34), PALETTE["path_light"])
    add_tree(draw, 15, 27)
    add_tree(draw, 33, 28)
    add_tree(draw, 24, 25)
    rect(draw, (8, 35, 14, 39), PALETTE["leaf_dark"])
    rect(draw, (9, 34, 13, 37), PALETTE["leaf_mid"])
    rect(draw, (34, 35, 40, 39), PALETTE["leaf_dark"])
    rect(draw, (35, 34, 39, 37), PALETTE["leaf_mid"])
    for x, y in ((11, 34), (37, 35), (16, 37), (32, 38)):
      pixel(draw, x, y, PALETTE["flower"])
    return frame


def draw_public() -> Image.Image:
    frame = make_frame()
    draw = ImageDraw.Draw(frame)
    add_ground_shadow(draw, 8, 41, 40, 44)
    rect(draw, (14, 39, 34, 42), PALETTE["path_dark"])
    rect(draw, (15, 38, 33, 40), PALETTE["path"])
    rect(draw, (11, 21, 14, 40), PALETTE["outline"])
    rect(draw, (12, 22, 13, 39), PALETTE["metal"])
    rect(draw, (34, 21, 37, 40), PALETTE["outline"])
    rect(draw, (35, 22, 36, 39), PALETTE["metal"])
    polygon(draw, [(8, 21), (14, 15), (34, 15), (40, 21)], PALETTE["outline"])
    polygon(draw, [(10, 20), (15, 16), (33, 16), (38, 20)], PALETTE["roof_green"])
    line(draw, (13, 19, 35, 19), PALETTE["roof_green_dark"])
    line(draw, (16, 16, 32, 16), PALETTE["roof_cool_light"])
    rect(draw, (16, 23, 32, 35), PALETTE["outline"])
    rect(draw, (17, 24, 31, 34), PALETTE["sign_bg"])
    rect(draw, (19, 26, 29, 27), PALETTE["trim"])
    rect(draw, (19, 29, 26, 30), PALETTE["sign_light"])
    rect(draw, (27, 29, 29, 32), PALETTE["wall_warm_light"])
    pixel(draw, 18, 25, PALETTE["roof_warm_light"])
    rect(draw, (14, 36, 34, 39), PALETTE["outline"])
    rect(draw, (15, 36, 33, 37), PALETTE["wall_stone_light"])
    rect(draw, (17, 38, 19, 40), PALETTE["metal_dark"])
    rect(draw, (29, 38, 31, 40), PALETTE["metal_dark"])
    return frame


def build_spritesheet() -> Image.Image:
    frames = [draw_home(), draw_shop(), draw_civic(), draw_workplace(), draw_park(), draw_public()]
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
