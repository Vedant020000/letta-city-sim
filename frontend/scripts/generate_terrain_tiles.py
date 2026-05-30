"""Generate the first-pass pixel-art terrain tileset for the frontend."""

from pathlib import Path
from PIL import Image, ImageDraw

TILE_SIZE = 16
COLUMNS = 4
ROWS = 4
OUTPUT_FILE = Path(__file__).resolve().parents[1] / "public" / "sprites" / "terrain-tiles.png"

# Muted, darkish city-sim palette.
PALETTE = {
    # Grass
    "grass_base": "#42583D",
    "grass_dark": "#344832",
    "grass_mid": "#506648",
    "grass_light": "#607453",
    "lush_base": "#3A573A",
    "lush_dark": "#2D472F",
    "lush_light": "#57724D",
    # Dirt and gravel
    "dirt_base": "#655544",
    "dirt_dark": "#514536",
    "dirt_mid": "#75624D",
    "dirt_light": "#84715A",
    "gravel_base": "#625E55",
    "gravel_dark": "#4E4B45",
    "gravel_light": "#777269",
    # Paths and plaza
    "path_base": "#777268",
    "path_dark": "#625E57",
    "path_light": "#898379",
    "stone_base": "#696B68",
    "stone_dark": "#555856",
    "stone_light": "#7B7D78",
    # Water
    "water_base": "#375764",
    "water_dark": "#2C4854",
    "water_mid": "#426875",
    "water_light": "#527987",
    # Foliage accents
    "leaf_dark": "#29422D",
    "leaf_mid": "#42633B",
    "leaf_light": "#5E7B4A",
    "trunk": "#554636",
    "flower_yellow": "#C3A95D",
    "flower_pink": "#A86D78",
    "flower_blue": "#738AA1",
    # Fence
    "fence_dark": "#3D403E",
    "fence_mid": "#62625C",
    "fence_light": "#7A786F",
    # Reserved filler tiles
    "filler_base": "#343B37",
    "filler_dark": "#2C332F",
}


def set_pixel(draw: ImageDraw.ImageDraw, x: int, y: int, color: str) -> None:
    draw.point((x, y), fill=color)


def fill_tile(draw: ImageDraw.ImageDraw, color: str) -> None:
    draw.rectangle((0, 0, TILE_SIZE - 1, TILE_SIZE - 1), fill=color)


def scatter_pattern(draw: ImageDraw.ImageDraw, coordinates: list[tuple[int, int]], color: str) -> None:
    for x, y in coordinates:
        set_pixel(draw, x, y, color)


def make_grass() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["grass_base"])
    scatter_pattern(draw, [(2, 3), (9, 2), (13, 5), (5, 8), (11, 10), (3, 13), (14, 14)], PALETTE["grass_dark"])
    scatter_pattern(draw, [(6, 2), (1, 7), (12, 7), (7, 12), (10, 15)], PALETTE["grass_mid"])
    return tile


def make_dirt() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["dirt_base"])
    scatter_pattern(draw, [(3, 2), (10, 3), (14, 6), (5, 7), (1, 11), (9, 12), (13, 14)], PALETTE["dirt_dark"])
    scatter_pattern(draw, [(6, 4), (12, 9), (4, 13), (8, 15)], PALETTE["dirt_mid"])
    return tile


def make_path() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["path_base"])
    for y in (3, 7, 11, 15):
        for x in range(0, TILE_SIZE, 4):
            set_pixel(draw, x, y, PALETTE["path_dark"])
    scatter_pattern(draw, [(2, 1), (10, 2), (6, 5), (14, 6), (3, 9), (11, 10), (7, 13)], PALETTE["path_light"])
    return tile


def make_water() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["water_base"])
    draw.line((2, 3, 6, 3), fill=PALETTE["water_mid"])
    draw.line((10, 3, 13, 3), fill=PALETTE["water_dark"])
    draw.line((5, 7, 11, 7), fill=PALETTE["water_light"])
    draw.line((0, 11, 4, 11), fill=PALETTE["water_dark"])
    draw.line((9, 12, 15, 12), fill=PALETTE["water_mid"])
    return tile


def make_grass_variant() -> Image.Image:
    tile = make_grass()
    draw = ImageDraw.Draw(tile)
    scatter_pattern(draw, [(4, 4), (5, 4), (12, 3), (8, 9), (9, 9), (2, 15)], PALETTE["grass_light"])
    scatter_pattern(draw, [(4, 5), (12, 4), (8, 10)], PALETTE["grass_dark"])
    return tile


def make_dirt_variant() -> Image.Image:
    tile = make_dirt()
    draw = ImageDraw.Draw(tile)
    scatter_pattern(draw, [(1, 4), (2, 4), (8, 6), (9, 6), (12, 11), (13, 11), (5, 15)], PALETTE["dirt_light"])
    scatter_pattern(draw, [(6, 1), (11, 5), (3, 9), (7, 13)], PALETTE["dirt_dark"])
    return tile


def make_path_variant() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["path_base"])
    for y in (1, 5, 9, 13):
        offset = 0 if y in (1, 9) else 2
        for x in range(offset, TILE_SIZE, 4):
            set_pixel(draw, x, y, PALETTE["path_dark"])
    scatter_pattern(draw, [(3, 3), (11, 4), (7, 7), (1, 12), (13, 14)], PALETTE["path_light"])
    return tile


def make_water_variant() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["water_base"])
    draw.line((1, 2, 5, 2), fill=PALETTE["water_dark"])
    draw.line((8, 4, 14, 4), fill=PALETTE["water_mid"])
    draw.line((3, 8, 8, 8), fill=PALETTE["water_light"])
    draw.line((11, 10, 15, 10), fill=PALETTE["water_dark"])
    draw.line((0, 14, 6, 14), fill=PALETTE["water_mid"])
    return tile


def make_lush_grass() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["lush_base"])
    scatter_pattern(draw, [(1, 2), (5, 1), (10, 2), (14, 4), (3, 6), (8, 5), (12, 8), (1, 10), (6, 11), (10, 13), (14, 12), (4, 15)], PALETTE["lush_dark"])
    scatter_pattern(draw, [(3, 3), (7, 2), (11, 6), (5, 8), (2, 13), (13, 15)], PALETTE["lush_light"])
    return tile


def make_gravel() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["gravel_base"])
    scatter_pattern(draw, [(2, 2), (7, 1), (12, 3), (4, 5), (9, 6), (14, 7), (1, 9), (6, 10), (11, 11), (4, 14), (13, 15)], PALETTE["gravel_dark"])
    scatter_pattern(draw, [(5, 2), (10, 4), (2, 7), (8, 9), (14, 12), (7, 14)], PALETTE["gravel_light"])
    return tile


def make_plaza() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["stone_base"])
    for coordinate in range(0, TILE_SIZE, 4):
        draw.line((coordinate, 0, coordinate, TILE_SIZE - 1), fill=PALETTE["stone_dark"])
        draw.line((0, coordinate, TILE_SIZE - 1, coordinate), fill=PALETTE["stone_dark"])
    scatter_pattern(draw, [(2, 2), (6, 2), (10, 6), (14, 10), (6, 14)], PALETTE["stone_light"])
    return tile


def make_reserved_filler() -> Image.Image:
    tile = Image.new("RGB", (TILE_SIZE, TILE_SIZE))
    draw = ImageDraw.Draw(tile)
    fill_tile(draw, PALETTE["filler_base"])
    scatter_pattern(draw, [(3, 3), (11, 3), (7, 7), (3, 11), (11, 11), (15, 15)], PALETTE["filler_dark"])
    return tile


def make_tree_base() -> Image.Image:
    tile = make_lush_grass()
    draw = ImageDraw.Draw(tile)
    draw.rectangle((5, 10, 10, 13), fill=PALETTE["leaf_dark"])
    draw.rectangle((7, 10, 8, 14), fill=PALETTE["trunk"])
    draw.point((9, 11), fill=PALETTE["leaf_mid"])
    draw.point((5, 12), fill=PALETTE["leaf_mid"])
    return tile


def make_flower_accent() -> Image.Image:
    tile = make_lush_grass()
    draw = ImageDraw.Draw(tile)
    flowers = [
        (3, 4, PALETTE["flower_yellow"]),
        (11, 3, PALETTE["flower_pink"]),
        (7, 9, PALETTE["flower_blue"]),
        (13, 12, PALETTE["flower_yellow"]),
        (3, 13, PALETTE["flower_pink"]),
    ]
    for x, y, color in flowers:
        set_pixel(draw, x, y, color)
        if y + 1 < TILE_SIZE:
            set_pixel(draw, x, y + 1, PALETTE["leaf_dark"])
    return tile


def make_fence_border() -> Image.Image:
    tile = make_grass()
    draw = ImageDraw.Draw(tile)
    draw.line((0, 7, TILE_SIZE - 1, 7), fill=PALETTE["fence_dark"])
    draw.line((0, 8, TILE_SIZE - 1, 8), fill=PALETTE["fence_mid"])
    for x in (2, 7, 12):
        draw.rectangle((x, 5, x + 1, 10), fill=PALETTE["fence_dark"])
        draw.point((x + 1, 6), fill=PALETTE["fence_light"])
    return tile


def build_tileset() -> Image.Image:
    tiles = [
        make_grass(), make_dirt(), make_path(), make_water(),
        make_grass_variant(), make_dirt_variant(), make_path_variant(), make_water_variant(),
        make_lush_grass(), make_gravel(), make_plaza(), make_reserved_filler(),
        make_tree_base(), make_flower_accent(), make_fence_border(), make_reserved_filler(),
    ]
    atlas = Image.new("RGB", (COLUMNS * TILE_SIZE, ROWS * TILE_SIZE), PALETTE["filler_base"])
    for index, tile in enumerate(tiles):
        column = index % COLUMNS
        row = index // COLUMNS
        atlas.paste(tile, (column * TILE_SIZE, row * TILE_SIZE))
    return atlas


def main() -> None:
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    atlas = build_tileset()
    atlas.save(OUTPUT_FILE, format="PNG")
    print(f"Saved {OUTPUT_FILE}")


if __name__ == "__main__":
    main()
