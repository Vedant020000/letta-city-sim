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
export type StructureKind = "home" | "shop" | "civic" | "workplace" | "park" | "public";

export type TileCoord = {
  tx: number;
  ty: number;
};

export type ChunkRoadTile = TileCoord & {
  mask: number;
};

export type ChunkLocationAnchor = TileCoord & {
  location: Location;
  region: DistrictKind;
};

export type ChunkStructure = TileCoord & {
  id: string;
  label: string;
  kind: StructureKind;
  region: DistrictKind;
  locationIds: string[];
  locationCount: number;
};

export type TownChunk = {
  key: string;
  cx: number;
  cy: number;
  roadTiles: ChunkRoadTile[];
  locations: ChunkLocationAnchor[];
  structures: ChunkStructure[];
  districtKind: DistrictKind;
};

export type ChunkWorld = {
  chunks: Map<string, TownChunk>;
  anchorsByLocationId: Map<string, ChunkLocationAnchor>;
  structuresById: Map<string, ChunkStructure>;
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

function roadMaskForTile(tile: TileCoord, roadTiles: Set<string>) {
  let mask = 0;
  if (roadTiles.has(roadTileKey(tile.tx, tile.ty - 1))) mask |= 1;
  if (roadTiles.has(roadTileKey(tile.tx + 1, tile.ty))) mask |= 2;
  if (roadTiles.has(roadTileKey(tile.tx, tile.ty + 1))) mask |= 4;
  if (roadTiles.has(roadTileKey(tile.tx - 1, tile.ty))) mask |= 8;
  return mask;
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

function isParkLikeLocation(location: Location) {
  const name = location.name.toLowerCase();
  return name.includes("park") || name.includes("garden") || name.includes("campground") || name.includes("forest");
}

export function regionForLocation(location: Location): DistrictKind {
  const id = location.id;
  const name = location.name.toLowerCase();

  if (location.kind === "home" || id.startsWith("lin_") || id.startsWith("home_")) return "home";
  if (location.kind === "business") return "commercial";
  if (location.kind === "civic") return "civic";
  if (location.kind === "workplace") return name.includes("bank") || name.includes("hall") || name.includes("library") ? "civic" : "commercial";
  if (location.kind === "public") return isParkLikeLocation(location) ? "park" : "civic";

  if (name.includes("cafe") || name.includes("shop") || name.includes("store") || name.includes("grocery") || name.includes("bakery") || name.includes("market")) return "commercial";
  if (isParkLikeLocation(location)) return "park";
  if (name.includes("hall") || name.includes("dorm") || name.includes("clinic") || name.includes("bank") || name.includes("motel") || name.includes("library")) return "civic";
  return "residential";
}

function structureKindForLocation(location: Location): StructureKind {
  switch (location.kind) {
    case "home":
      return "home";
    case "business":
      return "shop";
    case "civic":
      return "civic";
    case "workplace":
      return "workplace";
    case "public":
      return isParkLikeLocation(location) ? "park" : "public";
    default:
      return isParkLikeLocation(location) ? "park" : "public";
  }
}

function structureGroupKey(location: Location) {
  const id = location.id;

  if (id.startsWith("home_")) return id;
  if (id.startsWith("lin_")) return "lin_family_home";
  if (id.startsWith("hobbs_cafe_")) return "hobbs_cafe";
  if (id.startsWith("harvey_oak_")) return "harvey_oak_supermart";
  if (id.startsWith("oak_")) return "oak_hill_college";
  if (id.startsWith("smallville_library_")) return "smallville_library";
  if (id.startsWith("townhall_")) return "town_hall";
  if (id.startsWith("smallville_bank_")) return "smallville_bank";
  if (id.startsWith("smallville_motel_")) return "smallville_motel";
  if (id.startsWith("ville_park_") || id === "notice_board") return "ville_park";

  return id;
}

function structureLabelForKey(key: string, members: ChunkLocationAnchor[]) {
  switch (key) {
    case "lin_family_home":
      return "Lin Family Home";
    case "hobbs_cafe":
      return "Hobbs Cafe";
    case "harvey_oak_supermart":
      return "Harvey Oak Supermart";
    case "oak_hill_college":
      return "Oak Hill College";
    case "smallville_library":
      return "Smallville Library";
    case "town_hall":
      return "Town Hall";
    case "smallville_bank":
      return "Smallville Bank";
    case "smallville_motel":
      return "Smallville Motel";
    case "ville_park":
      return "Ville Park";
    default:
      return members[0]?.location.name ?? key;
  }
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
    structures: [],
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

function buildStructures(anchors: ChunkLocationAnchor[]) {
  const groups = new Map<string, ChunkLocationAnchor[]>();
  for (const anchor of anchors) {
    const key = structureGroupKey(anchor.location);
    const existing = groups.get(key) ?? [];
    existing.push(anchor);
    groups.set(key, existing);
  }

  const structures = new Map<string, ChunkStructure>();
  for (const [key, members] of groups.entries()) {
    const tx = Math.round(members.reduce((sum, member) => sum + member.tx, 0) / members.length);
    const ty = Math.round(members.reduce((sum, member) => sum + member.ty, 0) / members.length);
    const region = members.reduce<DistrictKind>((best, member) => (
      districtPriority(member.region) >= districtPriority(best) ? member.region : best
    ), members[0]?.region ?? "wild");

    structures.set(key, {
      id: key,
      label: structureLabelForKey(key, members),
      kind: structureKindForLocation(members[0].location),
      region,
      tx,
      ty,
      locationIds: members.map((member) => member.location.id),
      locationCount: members.length,
    });
  }

  return structures;
}

export function buildChunkWorld(locations: Location[]): ChunkWorld {
  if (locations.length === 0) {
    return {
      chunks: new Map(),
      anchorsByLocationId: new Map(),
      structuresById: new Map(),
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
  const structuresById = buildStructures(anchors);
  const roadTiles = new Set<string>();

  for (const [start, end] of roadConnectionPairs(anchors)) {
    stampRoadPath(start, end, roadTiles);
  }

  let minTx = Infinity;
  let maxTx = -Infinity;
  let minTy = Infinity;
  let maxTy = -Infinity;

  for (const structure of structuresById.values()) {
    const left = structure.tx - Math.floor(LOCATION_FOOTPRINT_TILES.width / 2) - 2;
    const right = structure.tx + Math.ceil(LOCATION_FOOTPRINT_TILES.width / 2) + 2;
    const top = structure.ty - Math.floor(LOCATION_FOOTPRINT_TILES.height / 2) - 2;
    const bottom = structure.ty + Math.ceil(LOCATION_FOOTPRINT_TILES.height / 2) + 3;
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

  for (const structure of structuresById.values()) {
    const cx = Math.floor(structure.tx / WORLD_CHUNK_SIZE);
    const cy = Math.floor(structure.ty / WORLD_CHUNK_SIZE);
    const key = chunkKey(cx, cy);
    const chunk = chunks.get(key) ?? emptyChunk(cx, cy);
    chunk.structures.push(structure);

    if (districtPriority(structure.region) >= districtPriority(chunk.districtKind)) {
      chunk.districtKind = structure.region;
    }

    chunks.set(key, chunk);
  }

  for (const roadKey of roadTiles) {
    const tile = parseRoadTileKey(roadKey);
    const cx = Math.floor(tile.tx / WORLD_CHUNK_SIZE);
    const cy = Math.floor(tile.ty / WORLD_CHUNK_SIZE);
    const key = chunkKey(cx, cy);
    const chunk = chunks.get(key) ?? emptyChunk(cx, cy);
    chunk.roadTiles.push({
      ...tile,
      mask: roadMaskForTile(tile, roadTiles),
    });

    if (chunk.districtKind === "wild") {
      chunk.districtKind = "residential";
    }

    chunks.set(key, chunk);
  }

  return {
    chunks,
    anchorsByLocationId,
    structuresById,
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
