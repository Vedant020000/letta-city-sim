import * as Phaser from "phaser";
import { Agent, Location } from "@/types/world";
import {
  buildChunkWorld,
  ChunkLocationAnchor,
  ChunkWorld,
  DistrictKind,
  LOCATION_FOOTPRINT_TILES,
  TownChunk,
  WORLD_CHUNK_SIZE,
  WORLD_TILE_SIZE,
} from "@/lib/chunk-world";

type TownSceneSnapshot = {
  agents: Agent[];
  locations: Location[];
};

const MARKER_TWEEN_MS = 700;

const LOCATION_COLORS: Record<DistrictKind, { bg: number; border: number; label: string }> = {
  residential: { bg: 0x1e3a5f, border: 0x3b82f6, label: "Residential" },
  commercial: { bg: 0x1a3d2e, border: 0x22c55e, label: "Commercial" },
  civic: { bg: 0x3b1f4a, border: 0xa855f7, label: "Civic" },
  park: { bg: 0x1a3a2a, border: 0x4ade80, label: "Park" },
  home: { bg: 0x3b2f1a, border: 0xf59e0b, label: "Home" },
  wild: { bg: 0x16351f, border: 0x355e3b, label: "Wild" },
};

const CHUNK_THEME: Record<DistrictKind, { base: number; alt: number; border: number; road: number }> = {
  residential: { base: 0x13283c, alt: 0x173149, border: 0x274864, road: 0x46515f },
  commercial: { base: 0x162f24, alt: 0x1b3a2c, border: 0x28553f, road: 0x4a5560 },
  civic: { base: 0x2a1f37, alt: 0x352747, border: 0x55406d, road: 0x4f5868 },
  park: { base: 0x133220, alt: 0x173f27, border: 0x2d6b40, road: 0x59614f },
  home: { base: 0x382717, alt: 0x43321e, border: 0x75511e, road: 0x60574d },
  wild: { base: 0x10261a, alt: 0x143020, border: 0x264c35, road: 0x4e594d },
};

const ROAD_MASK_TO_FRAME: Record<number, number> = {
  0: 0,
  1: 1,
  2: 2,
  3: 3,
  4: 4,
  5: 5,
  6: 6,
  7: 7,
  8: 8,
  9: 9,
  10: 10,
  11: 11,
  12: 12,
  13: 13,
  14: 14,
  15: 15,
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

function footprintBounds(anchor: ChunkLocationAnchor) {
  const center = tileCenter(anchor.tx, anchor.ty);
  const width = LOCATION_FOOTPRINT_TILES.width * WORLD_TILE_SIZE;
  const height = LOCATION_FOOTPRINT_TILES.height * WORLD_TILE_SIZE;

  return {
    x: center.x - width / 2,
    y: center.y - height / 2,
    width,
    height,
    centerX: center.x,
    centerY: center.y,
  };
}

function roadFrameForMask(mask: number) {
  return ROAD_MASK_TO_FRAME[mask] ?? 0;
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

    const terrain = this.add.graphics();
    for (let localTy = 0; localTy < WORLD_CHUNK_SIZE; localTy += 1) {
      for (let localTx = 0; localTx < WORLD_CHUNK_SIZE; localTx += 1) {
        const worldTx = chunk.cx * WORLD_CHUNK_SIZE + localTx;
        const worldTy = chunk.cy * WORLD_CHUNK_SIZE + localTy;
        const isAlt = (worldTx + worldTy) % 2 === 0;
        terrain.fillStyle(isAlt ? theme.alt : theme.base, 1);
        terrain.fillRect(localTx * WORLD_TILE_SIZE, localTy * WORLD_TILE_SIZE, WORLD_TILE_SIZE, WORLD_TILE_SIZE);
      }
    }

    terrain.lineStyle(2, theme.border, 0.85);
    terrain.strokeRect(0, 0, chunkPixelSize, chunkPixelSize);
    terrain.lineStyle(1, theme.border, 0.15);
    for (let offset = WORLD_TILE_SIZE; offset < chunkPixelSize; offset += WORLD_TILE_SIZE) {
      terrain.lineBetween(offset, 0, offset, chunkPixelSize);
      terrain.lineBetween(0, offset, chunkPixelSize, offset);
    }
    container.add(terrain);

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

    const chunkLabel = this.add.text(8, 6, `chunk ${chunk.cx}:${chunk.cy}`, {
      color: "#94a3b8",
      fontSize: "10px",
      fontFamily: "Inter, Arial, sans-serif",
    });
    chunkLabel.setAlpha(0.7);
    container.add(chunkLabel);

    for (const anchor of chunk.locations) {
      const colors = LOCATION_COLORS[anchor.region] ?? LOCATION_COLORS.residential;
      const bounds = footprintBounds(anchor);
      const localCenterX = bounds.centerX - chunk.cx * chunkPixelSize;
      const localCenterY = bounds.centerY - chunk.cy * chunkPixelSize;

      const block = this.add.rectangle(localCenterX, localCenterY, bounds.width, bounds.height, colors.bg);
      block.setStrokeStyle(2, colors.border);
      container.add(block);

      const nameText = this.add.text(localCenterX, localCenterY - 10, anchor.location.name, {
        color: "#e2e8f0",
        fontSize: "11px",
        fontFamily: "Inter, Arial, sans-serif",
        fontStyle: "bold",
        align: "center",
        wordWrap: { width: Math.max(bounds.width - 10, 40) },
      });
      nameText.setOrigin(0.5, 0.5);
      container.add(nameText);

      const tag = this.add.text(localCenterX, localCenterY + 14, colors.label, {
        color: `#${colors.border.toString(16).padStart(6, "0")}`,
        fontSize: "9px",
        fontFamily: "Inter, Arial, sans-serif",
        align: "center",
      });
      tag.setOrigin(0.5, 0.5);
      container.add(tag);

      const anchorOutline = this.add.rectangle(
        (anchor.tx - chunk.cx * WORLD_CHUNK_SIZE) * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
        (anchor.ty - chunk.cy * WORLD_CHUNK_SIZE) * WORLD_TILE_SIZE + WORLD_TILE_SIZE / 2,
        WORLD_TILE_SIZE,
        WORLD_TILE_SIZE,
        colors.border,
        0.18,
      );
      anchorOutline.setStrokeStyle(1, colors.border, 0.55);
      container.add(anchorOutline);
    }

    return container;
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
