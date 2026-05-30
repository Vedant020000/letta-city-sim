import { Location } from "@/types/world";

export const WORLD_TILE_SIZE = 16;
export const WORLD_CHUNK_SIZE = 16;
export const LOCATION_FOOTPRINT_TILES = {
  width: 6,
  height: 4,
} as const;

const MAP_TO_TILE_SCALE = 6;
const WORLD_PADDING_TILES = 6;
const MAX_NEIGHBORS_PER_LOCATION = 2;

export type DistrictKind = "wild" | "residential" | "commercial" | "civic" | "park" | "home";

export type TileCoord = {
  tx: number;
  ty: number;
};

export type ChunkLocationAnchor = TileCoord & {
  location: Location;
  region: DistrictKind;
};

export type TownChunk = {
  key: string;
  cx: number;
  cy: number;
  roadTiles: TileCoord[];
  locations: ChunkLocationAnchor[];
  districtKind: DistrictKind;
};

export type ChunkWorld = {
  chunks: Map<string, TownChunk>;
  anchorsByLocationId: Map<string, ChunkLocationAnchor>;
  minTx: number;
  maxTx: number;
  minTy: number;
  maxTy: number;
  minCx: number;
  maxCx: number;
  minCy: number;
  maxCy: number;
};

type DistanceCandidate = {
  anchor: ChunkLocationAnchor;
  distance: number;
};

function chunkKey(cx: number, cy: number) {
  return `${cx}:${cy}`;
}

function roadTileKey(tx: number, ty: number) {
  return `${tx}:${ty}`;
}

function parseRoadTileKey(key: string): TileCoord {
  const [tx, ty] = key.split(":").map(Number);
  return { tx, ty };
}

function median(values: number[]) {
  if (values.length === 0) {
    return 0;
  }

  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

export function regionForLocation(location: Location): DistrictKind {
  const id = location.id;
  const name = location.name.toLowerCase();

  if (id.startsWith("lin_") || id.startsWith("home_")) return "home";
  if (name.includes("cafe") || name.includes("shop") || name.includes("store") || name.includes("grocery") || name.includes("bakery") || name.includes("market")) return "commercial";
  if (name.includes("park") || name.includes("garden") || name.includes("campground")) return "park";
  if (name.includes("hall") || name.includes("dorm") || name.includes("clinic") || name.includes("bank") || name.includes("motel") || name.includes("library")) return "civic";
  return "residential";
}

function roadConnectionPairs(anchors: ChunkLocationAnchor[]) {
  const pairMap = new Map<string, [ChunkLocationAnchor, ChunkLocationAnchor]>();
  const nearestDistances: number[] = [];
  const neighborsByAnchor = new Map<string, DistanceCandidate[]>();

  for (const anchor of anchors) {
    const neighbors = anchors
      .filter((candidate) => candidate.location.id !== anchor.location.id)
      .map((candidate) => ({
        anchor: candidate,
        distance: Math.hypot(candidate.tx - anchor.tx, candidate.ty - anchor.ty),
      }))
      .sort((a, b) => a.distance - b.distance);

    neighborsByAnchor.set(anchor.location.id, neighbors);

    if (neighbors[0]) {
      nearestDistances.push(neighbors[0].distance);
    }
  }

  const nearestMedian = median(nearestDistances);
  const maxDistance = Math.max(10, nearestMedian * 1.85);

  for (const anchor of anchors) {
    const neighbors = neighborsByAnchor.get(anchor.location.id) ?? [];

    neighbors.slice(0, MAX_NEIGHBORS_PER_LOCATION).forEach((candidate, index) => {
      if (index > 0 && candidate.distance > maxDistance) {
        return;
      }

      const pair = [anchor.location.id, candidate.anchor.location.id].sort().join("|");
      if (!pairMap.has(pair)) {
        pairMap.set(pair, [anchor, candidate.anchor]);
      }
    });
  }

  return [...pairMap.values()];
}

function stampRoadPath(start: TileCoord, end: TileCoord, roadTiles: Set<string>) {
  let tx = start.tx;
  let ty = start.ty;
  roadTiles.add(roadTileKey(tx, ty));

  while (tx !== end.tx) {
    tx += Math.sign(end.tx - tx);
    roadTiles.add(roadTileKey(tx, ty));
  }

  while (ty !== end.ty) {
    ty += Math.sign(end.ty - ty);
    roadTiles.add(roadTileKey(tx, ty));
  }
}

function emptyChunk(cx: number, cy: number): TownChunk {
  return {
    key: chunkKey(cx, cy),
    cx,
    cy,
    roadTiles: [],
    locations: [],
    districtKind: "wild",
  };
}

function districtPriority(region: DistrictKind) {
  switch (region) {
    case "park":
      return 5;
    case "commercial":
      return 4;
    case "civic":
      return 3;
    case "home":
      return 2;
    case "residential":
      return 1;
    default:
      return 0;
  }
}

export function buildChunkWorld(locations: Location[]): ChunkWorld {
  if (locations.length === 0) {
    return {
      chunks: new Map(),
      anchorsByLocationId: new Map(),
      minTx: 0,
      maxTx: 0,
      minTy: 0,
      maxTy: 0,
      minCx: 0,
      maxCx: 0,
      minCy: 0,
      maxCy: 0,
    };
  }

  const minMapX = Math.min(...locations.map((location) => location.map_x));
  const minMapY = Math.min(...locations.map((location) => location.map_y));

  const anchors = locations.map<ChunkLocationAnchor>((location) => ({
    location,
    tx: Math.round((location.map_x - minMapX) * MAP_TO_TILE_SCALE) + WORLD_PADDING_TILES,
    ty: Math.round((location.map_y - minMapY) * MAP_TO_TILE_SCALE) + WORLD_PADDING_TILES,
    region: regionForLocation(location),
  }));

  const anchorsByLocationId = new Map(anchors.map((anchor) => [anchor.location.id, anchor]));
  const roadTiles = new Set<string>();

  for (const [start, end] of roadConnectionPairs(anchors)) {
    stampRoadPath(start, end, roadTiles);
  }

  let minTx = Infinity;
  let maxTx = -Infinity;
  let minTy = Infinity;
  let maxTy = -Infinity;

  for (const anchor of anchors) {
    const left = anchor.tx - Math.floor(LOCATION_FOOTPRINT_TILES.width / 2) - 2;
    const right = anchor.tx + Math.ceil(LOCATION_FOOTPRINT_TILES.width / 2) + 2;
    const top = anchor.ty - Math.floor(LOCATION_FOOTPRINT_TILES.height / 2) - 2;
    const bottom = anchor.ty + Math.ceil(LOCATION_FOOTPRINT_TILES.height / 2) + 3;

    minTx = Math.min(minTx, left);
    maxTx = Math.max(maxTx, right);
    minTy = Math.min(minTy, top);
    maxTy = Math.max(maxTy, bottom);
  }

  for (const key of roadTiles) {
    const tile = parseRoadTileKey(key);
    minTx = Math.min(minTx, tile.tx - 1);
    maxTx = Math.max(maxTx, tile.tx + 1);
    minTy = Math.min(minTy, tile.ty - 1);
    maxTy = Math.max(maxTy, tile.ty + 1);
  }

  minTx = Math.max(0, minTx);
  minTy = Math.max(0, minTy);

  const minCx = Math.floor(minTx / WORLD_CHUNK_SIZE);
  const maxCx = Math.floor(maxTx / WORLD_CHUNK_SIZE);
  const minCy = Math.floor(minTy / WORLD_CHUNK_SIZE);
  const maxCy = Math.floor(maxTy / WORLD_CHUNK_SIZE);

  const chunks = new Map<string, TownChunk>();
  for (let cy = minCy; cy <= maxCy; cy += 1) {
    for (let cx = minCx; cx <= maxCx; cx += 1) {
      const chunk = emptyChunk(cx, cy);
      chunks.set(chunk.key, chunk);
    }
  }

  for (const anchor of anchors) {
    const cx = Math.floor(anchor.tx / WORLD_CHUNK_SIZE);
    const cy = Math.floor(anchor.ty / WORLD_CHUNK_SIZE);
    const key = chunkKey(cx, cy);
    const chunk = chunks.get(key) ?? emptyChunk(cx, cy);
    chunk.locations.push(anchor);

    if (districtPriority(anchor.region) >= districtPriority(chunk.districtKind)) {
      chunk.districtKind = anchor.region;
    }

    chunks.set(key, chunk);
  }

  for (const roadKey of roadTiles) {
    const tile = parseRoadTileKey(roadKey);
    const cx = Math.floor(tile.tx / WORLD_CHUNK_SIZE);
    const cy = Math.floor(tile.ty / WORLD_CHUNK_SIZE);
    const key = chunkKey(cx, cy);
    const chunk = chunks.get(key) ?? emptyChunk(cx, cy);
    chunk.roadTiles.push(tile);

    if (chunk.districtKind === "wild") {
      chunk.districtKind = "residential";
    }

    chunks.set(key, chunk);
  }

  return {
    chunks,
    anchorsByLocationId,
    minTx,
    maxTx,
    minTy,
    maxTy,
    minCx,
    maxCx,
    minCy,
    maxCy,
  };
}
