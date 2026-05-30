import * as Phaser from "phaser";
import { Agent, Location } from "@/types/world";
import {
  buildChunkWorld,
  ChunkStructure,
  ChunkWorld,
  DistrictKind,
  LOCATION_FOOTPRINT_TILES,
  StructureKind,
  TownChunk,
  WORLD_CHUNK_SIZE,
  WORLD_TILE_SIZE,
} from "@/lib/chunk-world";

type TownSceneSnapshot = {
  agents: Agent[];
  locations: Location[];
};

const MARKER_TWEEN_MS = 700;

const CHUNK_THEME: Record<DistrictKind, { terrainFrames: number[]; border: number }> = {
  residential: { terrainFrames: [0, 0, 4, 1], border: 0x274864 },
  commercial: { terrainFrames: [1, 5, 9, 2, 6], border: 0x28553f },
  civic: { terrainFrames: [10, 10, 11, 9, 6], border: 0x55406d },
  park: { terrainFrames: [8, 8, 13, 12, 4], border: 0x2d6b40 },
  home: { terrainFrames: [0, 4, 0, 2], border: 0x75511e },
  wild: { terrainFrames: [0, 4, 8, 12, 13], border: 0x264c35 },
};

const ROAD_MASK_TO_FRAME: Record<number, number> = {
  0: 0, 1: 1, 2: 2, 3: 3,
  4: 4, 5: 5, 6: 6, 7: 7,
  8: 8, 9: 9, 10: 10, 11: 11,
  12: 12, 13: 13, 14: 14, 15: 15,
};

const STRUCTURE_FRAME_BY_KIND: Record<StructureKind, number> = {
  home: 0,
  shop: 1,
  civic: 2,
  workplace: 3,
  park: 4,
  public: 5,
};

const STRUCTURE_LABEL_BY_KIND: Record<StructureKind, string> = {
  home: "Home",
  shop: "Business",
  civic: "Civic",
  workplace: "Workplace",
  park: "Park",
  public: "Public",
};

const PARCEL_STYLE_BY_KIND: Record<StructureKind, { fill: number; border: number; widthTiles: number; heightTiles: number; showParcel: boolean }> = {
  home: { fill: 0x2d2317, border: 0xf59e0b, widthTiles: 5, heightTiles: 3, showParcel: true },
  shop: { fill: 0x1f2a20, border: 0x22c55e, widthTiles: 5, heightTiles: 3, showParcel: true },
  civic: { fill: 0x292037, border: 0xa855f7, widthTiles: 6, heightTiles: 4, showParcel: true },
  workplace: { fill: 0x242830, border: 0x94a3b8, widthTiles: 6, heightTiles: 4, showParcel: true },
  park: { fill: 0x183121, border: 0x4ade80, widthTiles: 5, heightTiles: 4, showParcel: false },
  public: { fill: 0x2b2d28, border: 0xd1d5db, widthTiles: 5, heightTiles: 3, showParcel: true },
};

function colorForAgent(agentId: string) {
  const palette = [0x3b82f6, 0xef4444, 0x22c55e, 0xa855f7, 0xf97316, 0x06b6d4, 0xec4899, 0xeab308];
  let hash = 0;
  for (const char of agentId) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return palette[hash % palette.length];
}

function getInitials(name: string) {
  return name
    .split(" ")
    .map((part) => part[0])
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

function stateIcon(state: string) {
  switch (state) {
    case "sleeping":
      return "Z";
    case "idle":
      return ".";
    case "walking":
      return ">";
    default:
      return "*";
  }
}

function chunkKey(cx: number, cy: number) {
  return `${cx}:${cy}`;
}

function tileCenter(tx: number, ty: number) {
  return {
    x: tx * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
    y: ty * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
  };
}

function roadFrameForMask(mask: number) {
  return ROAD_MASK_TO_FRAME[mask] ?? 0;
}

function terrainFrameForTile(district: DistrictKind, tx: number, ty: number) {
  const theme = CHUNK_THEME[district] ?? CHUNK_THEME.wild;
  const hash = (((tx * 92837111) ^ (ty * 689287499) ^ ((tx + 17) * (ty + 31) * 283923)) >>> 0);
  return theme.terrainFrames[hash % theme.terrainFrames.length];
}

function parcelBoundsForStructure(structure: ChunkStructure) {
  const style = PARCEL_STYLE_BY_KIND[structure.kind];
  const center = tileCenter(structure.tx, structure.ty);
  const width = style.widthTiles * WORLD_TILE_SIZE;
  const height = style.heightTiles * WORLD_TILE_SIZE;
  return {
    ...style,
    width,
    height,
    centerX: center.x,
    centerY: center.y,
  };
}

function metaLabelForStructure(structure: ChunkStructure) {
  return structure.locationCount > 1
    ? `${STRUCTURE_LABEL_BY_KIND[structure.kind]} · ${structure.locationCount} rooms`
    : STRUCTURE_LABEL_BY_KIND[structure.kind];
}

export class TownScene extends Phaser.Scene {
  private snapshot: TownSceneSnapshot = { agents: [], locations: [] };
  private chunkWorld: ChunkWorld | null = null;
  private worldLayer!: Phaser.GameObjects.Container;
  private chunkLayer!: Phaser.GameObjects.Container;
  private markerLayer!: Phaser.GameObjects.Container;
  private chunkContainers = new Map<string, Phaser.GameObjects.Container>();
  private agentMarkers = new Map<string, Phaser.GameObjects.Container>();
  private selectedAgentId: string | null = null;
  private onAgentClick: ((agentId: string) => void) | null = null;

  constructor() {
    super("TownScene");
  }

  setOnAgentClick(callback: (agentId: string) => void) {
    this.onAgentClick = callback;
  }

  setSelectedAgent(agentId: string | null) {
    this.selectedAgentId = agentId;
    this.renderAgentMarkers();
  }

  preload() {
    this.load.spritesheet("road-tiles", "/sprites/road-tiles.png", {
      frameWidth: WORLD_TILE_SIZE,
      frameHeight: WORLD_TILE_SIZE,
    });
    this.load.spritesheet("terrain-tiles", "/sprites/terrain-tiles.png", {
      frameWidth: WORLD_TILE_SIZE,
      frameHeight: WORLD_TILE_SIZE,
    });
    this.load.spritesheet("structure-tiles", "/sprites/structure-tiles.png", {
      frameWidth: 48,
      frameHeight: 48,
    });
  }

  create() {
    this.cameras.main.setBackgroundColor("#0b1220");
    this.cameras.main.roundPixels = true;
    this.worldLayer = this.add.container(0, 0);
    this.chunkLayer = this.add.container(0, 0);
    this.markerLayer = this.add.container(0, 0);
    this.worldLayer.add([this.chunkLayer, this.markerLayer]);

    this.scale.on("resize", this.renderSnapshot, this);
    this.events.once(Phaser.Scenes.Events.SHUTDOWN, () => {
      this.scale.off("resize", this.renderSnapshot, this);
    });

    this.renderSnapshot();
  }

  applySnapshot(snapshot: TownSceneSnapshot) {
    this.snapshot = snapshot;
    this.chunkWorld = buildChunkWorld(snapshot.locations);
    this.clearRenderedChunks();

    if (this.worldLayer) {
      this.renderSnapshot();
    }
  }

  private buildAgentsByLocation(): Map<string, Agent[]> {
    const map = new Map<string, Agent[]>();
    for (const agent of this.snapshot.agents) {
      const existing = map.get(agent.current_location_id) ?? [];
      existing.push(agent);
      map.set(agent.current_location_id, existing);
    }
    return map;
  }

  private renderSnapshot() {
    if (!this.chunkLayer || !this.markerLayer) {
      return;
    }

    if (!this.chunkWorld || this.chunkWorld.chunks.size === 0) {
      this.clearRenderedChunks();
      this.clearAgentMarkers();
      return;
    }

    this.configureCamera();
    this.renderVisibleChunks();
    this.renderAgentMarkers();
  }

  private configureCamera() {
    if (!this.chunkWorld) {
      return;
    }

    const worldWidth = (this.chunkWorld.maxTx + 2) * WORLD_TILE_SIZE;
    const worldHeight = (this.chunkWorld.maxTy + 2) * WORLD_TILE_SIZE;
    this.cameras.main.setBounds(0, 0, worldWidth, worldHeight);

    const worldCenterX = ((this.chunkWorld.minTx + this.chunkWorld.maxTx + 1) * WORLD_TILE_SIZE) / 2;
    const worldCenterY = ((this.chunkWorld.minTy + this.chunkWorld.maxTy + 1) * WORLD_TILE_SIZE) / 2;
    this.cameras.main.centerOn(worldCenterX, worldCenterY);
  }

  private renderVisibleChunks() {
    if (!this.chunkWorld) {
      return;
    }

    const chunkPixelSize = WORLD_CHUNK_SIZE * WORLD_TILE_SIZE;
    const worldView = this.cameras.main.worldView;
    const minCx = Math.max(this.chunkWorld.minCx, Math.floor(worldView.left / chunkPixelSize) - 1);
    const maxCx = Math.min(this.chunkWorld.maxCx, Math.floor(worldView.right / chunkPixelSize) + 1);
    const minCy = Math.max(this.chunkWorld.minCy, Math.floor(worldView.top / chunkPixelSize) - 1);
    const maxCy = Math.min(this.chunkWorld.maxCy, Math.floor(worldView.bottom / chunkPixelSize) + 1);

    const visibleChunkKeys = new Set<string>();
    for (let cy = minCy; cy <= maxCy; cy += 1) {
      for (let cx = minCx; cx <= maxCx; cx += 1) {
        const key = chunkKey(cx, cy);
        const chunk = this.chunkWorld.chunks.get(key);
        if (!chunk) {
          continue;
        }

        visibleChunkKeys.add(key);
        if (!this.chunkContainers.has(key)) {
          const container = this.createChunkContainer(chunk);
          this.chunkLayer.add(container);
          this.chunkContainers.set(key, container);
        }
      }
    }

    for (const [key, container] of this.chunkContainers.entries()) {
      if (!visibleChunkKeys.has(key)) {
        container.destroy(true);
        this.chunkContainers.delete(key);
      }
    }
  }

  private createChunkContainer(chunk: TownChunk) {
    const chunkPixelSize = WORLD_CHUNK_SIZE * WORLD_TILE_SIZE;
    const container = this.add.container(chunk.cx * chunkPixelSize, chunk.cy * chunkPixelSize);
    const theme = CHUNK_THEME[chunk.districtKind] ?? CHUNK_THEME.wild;

    for (let localTy = 0; localTy < WORLD_CHUNK_SIZE; localTy += 1) {
      for (let localTx = 0; localTx < WORLD_CHUNK_SIZE; localTx += 1) {
        const worldTx = chunk.cx * WORLD_CHUNK_SIZE + localTx;
        const worldTy = chunk.cy * WORLD_CHUNK_SIZE + localTy;
        const terrainTile = this.add.image(
          localTx * WORLD_TILE_SIZE,
          localTy * WORLD_TILE_SIZE,
          "terrain-tiles",
          terrainFrameForTile(chunk.districtKind, worldTx, worldTy),
        );
        terrainTile.setOrigin(0, 0);
        container.add(terrainTile);
      }
    }

    for (const roadTile of chunk.roadTiles) {
      const localTx = roadTile.tx - chunk.cx * WORLD_CHUNK_SIZE;
      const localTy = roadTile.ty - chunk.cy * WORLD_CHUNK_SIZE;
      const roadSprite = this.add.image(
        localTx * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
        localTy * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
        "road-tiles",
        roadFrameForMask(roadTile.mask),
      );
      roadSprite.setDisplaySize(WORLD_TILE_SIZE, WORLD_TILE_SIZE);
      container.add(roadSprite);
    }

    for (const structure of chunk.structures) {
      this.renderStructure(container, chunk, structure);
    }

    const overlay = this.add.graphics();
    overlay.lineStyle(2, theme.border, 0.85);
    overlay.strokeRect(0, 0, chunkPixelSize, chunkPixelSize);
    overlay.lineStyle(1, theme.border, 0.12);
    for (let offset = WORLD_TILE_SIZE; offset < chunkPixelSize; offset += WORLD_TILE_SIZE) {
      overlay.lineBetween(offset, 0, offset, chunkPixelSize);
      overlay.lineBetween(0, offset, chunkPixelSize, offset);
    }
    container.add(overlay);

    const chunkLabel = this.add.text(8, 6, `chunk ${chunk.cx}:${chunk.cy}`, {
      color: "#94a3b8",
      fontSize: "10px",
      fontFamily: "Inter, Arial, sans-serif",
    });
    chunkLabel.setAlpha(0.65);
    container.add(chunkLabel);

    return container;
  }

  private renderStructure(container: Phaser.GameObjects.Container, chunk: TownChunk, structure: ChunkStructure) {
    const chunkPixelSize = WORLD_CHUNK_SIZE * WORLD_TILE_SIZE;
    const parcel = parcelBoundsForStructure(structure);
    const localCenterX = parcel.centerX - chunk.cx * chunkPixelSize;
    const localCenterY = parcel.centerY - chunk.cy * chunkPixelSize;

    if (parcel.showParcel) {
      const pad = this.add.rectangle(localCenterX, localCenterY + 4, parcel.width, parcel.height, parcel.fill, 0.75);
      pad.setStrokeStyle(2, parcel.border, 0.95);
      container.add(pad);

      const walkway = this.add.rectangle(localCenterX, localCenterY + parcel.height / 2 - WORLD_TILE_SIZE / 2, WORLD_TILE_SIZE, WORLD_TILE_SIZE, parcel.border, 0.18);
      walkway.setStrokeStyle(1, parcel.border, 0.45);
      container.add(walkway);
    }

    const structureSprite = this.add.image(
      localCenterX,
      localCenterY - (structure.kind === "park" ? 4 : 8),
      "structure-tiles",
      STRUCTURE_FRAME_BY_KIND[structure.kind],
    );
    structureSprite.setOrigin(0.5, 0.5);
    container.add(structureSprite);

    if (structure.locationCount > 1) {
      const roomDots = this.add.graphics();
      roomDots.fillStyle(0xe2e8f0, 0.9);
      const dotSpacing = 7;
      const startX = localCenterX - ((structure.locationCount - 1) * dotSpacing) / 2;
      for (let index = 0; index < structure.locationCount; index += 1) {
        roomDots.fillCircle(startX + index * dotSpacing, localCenterY + 16, 1.5);
      }
      container.add(roomDots);
    }

    const nameText = this.add.text(localCenterX, localCenterY + 26, structure.label, {
      color: "#e2e8f0",
      fontSize: "11px",
      fontFamily: "Inter, Arial, sans-serif",
      fontStyle: "bold",
      align: "center",
      wordWrap: { width: Math.max(LOCATION_FOOTPRINT_TILES.width * WORLD_TILE_SIZE, 72) },
    });
    nameText.setOrigin(0.5, 0.5);
    container.add(nameText);

    const metaText = this.add.text(localCenterX, localCenterY + 40, metaLabelForStructure(structure), {
      color: "#94a3b8",
      fontSize: "9px",
      fontFamily: "Inter, Arial, sans-serif",
      align: "center",
    });
    metaText.setOrigin(0.5, 0.5);
    container.add(metaText);
  }

  private renderAgentMarkers() {
    if (!this.chunkWorld) {
      this.clearAgentMarkers();
      return;
    }

    const agentsByLocation = this.buildAgentsByLocation();
    const seenAgents = new Set<string>();
    const markerBaseOffsetY = (LOCATION_FOOTPRINT_TILES.height * WORLD_TILE_SIZE) / 2 + 12;

    for (const anchor of this.chunkWorld.anchorsByLocationId.values()) {
      const locationAgents = agentsByLocation.get(anchor.location.id) ?? [];
      const center = tileCenter(anchor.tx, anchor.ty);

      locationAgents.forEach((agent, index) => {
        const offsetX = -20 + (index % 3) * 20;
        const offsetY = markerBaseOffsetY + Math.floor(index / 3) * 22;
        seenAgents.add(agent.id);
        this.upsertAgentMarker(agent, center.x + offsetX, center.y + offsetY);
      });
    }

    for (const [agentId, marker] of this.agentMarkers.entries()) {
      if (!seenAgents.has(agentId)) {
        this.tweens.killTweensOf(marker);
        marker.destroy(true);
        this.agentMarkers.delete(agentId);
      }
    }
  }

  private upsertAgentMarker(agent: Agent, x: number, y: number) {
    const isSelected = this.selectedAgentId === agent.id;
    let marker = this.agentMarkers.get(agent.id);

    if (!marker) {
      marker = this.add.container(x, y);

      const circle = this.add.circle(0, 0, 10, colorForAgent(agent.id));
      circle.setStrokeStyle(isSelected ? 4 : 2, isSelected ? 0xfbbf24 : 0x0f172a);
      marker.add(circle);

      const initials = this.add.text(0, 0, getInitials(agent.name), {
        color: "#ffffff",
        fontSize: "9px",
        fontFamily: "Inter, Arial, sans-serif",
        fontStyle: "bold",
      });
      initials.setOrigin(0.5, 0.5);
      marker.add(initials);

      const stateText = this.add.text(0, -16, stateIcon(agent.state), {
        color: "#e2e8f0",
        fontSize: "10px",
        fontFamily: "Inter, Arial, sans-serif",
      });
      stateText.setOrigin(0.5, 0.5);
      marker.add(stateText);

      circle.setInteractive({ useHandCursor: true });
      circle.on("pointerdown", () => {
        this.onAgentClick?.(agent.id);
      });

      this.markerLayer.add(marker);
      this.agentMarkers.set(agent.id, marker);
      return;
    }

    const circle = marker.getAt(0) as Phaser.GameObjects.Arc | undefined;
    const stateText = marker.getAt(2) as Phaser.GameObjects.Text | undefined;

    if (circle) {
      circle.setStrokeStyle(isSelected ? 4 : 2, isSelected ? 0xfbbf24 : 0x0f172a);
    }

    if (stateText) {
      stateText.setText(stateIcon(agent.state));
    }

    this.tweens.killTweensOf(marker);
    const distance = Phaser.Math.Distance.Between(marker.x, marker.y, x, y);
    if (distance < 1) {
      marker.setPosition(x, y);
      return;
    }

    this.tweens.add({
      targets: marker,
      x,
      y,
      duration: MARKER_TWEEN_MS,
      ease: "Sine.easeInOut",
    });
  }

  private clearRenderedChunks() {
    for (const container of this.chunkContainers.values()) {
      container.destroy(true);
    }
    this.chunkContainers.clear();
  }

  private clearAgentMarkers() {
    for (const marker of this.agentMarkers.values()) {
      this.tweens.killTweensOf(marker);
      marker.destroy(true);
    }
    this.agentMarkers.clear();
  }
}
