import * as Phaser from "phaser";
import { Agent, Location } from "@/types/world";

type TownSceneSnapshot = {
  agents: Agent[];
  locations: Location[];
};

type LocationAnchor = {
  location: Location;
  x: number;
  y: number;
};

const PADDING_X = 120;
const PADDING_Y = 80;
const TILE_W = 110;
const TILE_H = 72;
const MARKER_TWEEN_MS = 700;

// Region colors — warm, distinct, readable on dark
const REGION_COLORS: Record<string, { bg: number; border: number; label: string }> = {
  residential: { bg: 0x1e3a5f, border: 0x3b82f6, label: "Residential" },
  commercial:  { bg: 0x1a3d2e, border: 0x22c55e, label: "Commercial" },
  civic:       { bg: 0x3b1f4a, border: 0xa855f7, label: "Civic" },
  park:        { bg: 0x1a3a2a, border: 0x4ade80, label: "Park" },
  home:        { bg: 0x3b2f1a, border: 0xf59e0b, label: "Home" },
  default:     { bg: 0x1e293b, border: 0x64748b, label: "" },
};

function regionForLocation(location: Location) {
  const id = location.id;
  const name = location.name.toLowerCase();
  if (id.startsWith("lin_") || id.startsWith("home_")) return "home";
  if (name.includes("cafe") || name.includes("shop") || name.includes("store") || name.includes("grocery") || name.includes("bakery") || name.includes("market")) return "commercial";
  if (name.includes("park") || name.includes("garden") || name.includes("campground")) return "park";
  if (name.includes("hall") || name.includes("dorm") || name.includes("clinic") || name.includes("bank") || name.includes("motel") || name.includes("library")) return "civic";
  return "residential";
}

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

function stateIcon(state: string): string {
  switch (state) {
    case "sleeping": return "💤";
    case "idle": return "◦";
    case "walking": return "→";
    default: return "•";
  }
}

export class TownScene extends Phaser.Scene {
  private snapshot: TownSceneSnapshot = { agents: [], locations: [] };
  private worldLayer!: Phaser.GameObjects.Container;
  private staticLayer!: Phaser.GameObjects.Container;
  private markerLayer!: Phaser.GameObjects.Container;
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
    this.renderAgentMarkers(
      this.buildLocationAnchors(),
      this.buildAgentsByLocation(),
    );
  }

  create() {
    this.cameras.main.setBackgroundColor("#0f172a");
    this.worldLayer = this.add.container(0, 0);
    this.staticLayer = this.add.container(0, 0);
    this.markerLayer = this.add.container(0, 0);
    this.worldLayer.add([this.staticLayer, this.markerLayer]);
    this.scale.on("resize", this.renderSnapshot, this);
    this.events.once(Phaser.Scenes.Events.SHUTDOWN, () => {
      this.scale.off("resize", this.renderSnapshot, this);
    });
    this.renderSnapshot();
  }

  applySnapshot(snapshot: TownSceneSnapshot) {
    this.snapshot = snapshot;
    if (this.worldLayer) {
      this.renderSnapshot();
    }
  }

  private buildLocationAnchors(): LocationAnchor[] {
    const { locations } = this.snapshot;
    if (locations.length === 0) return [];

    const minX = Math.min(...locations.map((l) => l.map_x));
    const maxX = Math.max(...locations.map((l) => l.map_x));
    const minY = Math.min(...locations.map((l) => l.map_y));
    const maxY = Math.max(...locations.map((l) => l.map_y));

    const viewportWidth = this.scale.width || this.game.canvas.width || 960;
    const viewportHeight = this.scale.height || this.game.canvas.height || 620;
    const spreadX = Math.max(maxX - minX, 1);
    const spreadY = Math.max(maxY - minY, 1);
    const availableWidth = Math.max(viewportWidth - PADDING_X * 2, 1);
    const availableHeight = Math.max(viewportHeight - PADDING_Y * 2, 1);
    const mapScale = Math.min(availableWidth / spreadX, availableHeight / spreadY, 1);

    return locations.map((location) => ({
      location,
      x: (location.map_x - minX) * mapScale + PADDING_X,
      y: (location.map_y - minY) * mapScale + PADDING_Y,
    }));
  }

  private buildAgentsByLocation(): Map<string, Agent[]> {
    const map = new Map<string, Agent[]>();
    for (const agent of this.snapshot.agents) {
      const existing = map.get(agent.current_location_id) || [];
      existing.push(agent);
      map.set(agent.current_location_id, existing);
    }
    return map;
  }

  private renderSnapshot() {
    if (!this.staticLayer || !this.markerLayer) return;
    this.staticLayer.removeAll(true);

    const { locations } = this.snapshot;
    if (locations.length === 0) {
      this.clearAgentMarkers();
      return;
    }

    const viewportWidth = this.scale.width || this.game.canvas.width || 960;
    const viewportHeight = this.scale.height || this.game.canvas.height || 620;
    this.cameras.main.setBounds(0, 0, viewportWidth, viewportHeight);

    // Dark background
    const bg = this.add.rectangle(viewportWidth / 2, viewportHeight / 2, viewportWidth, viewportHeight, 0x0f172a);
    this.staticLayer.add(bg);

    // Subtle grid
    const grid = this.add.graphics();
    grid.lineStyle(1, 0x1e293b, 0.6);
    for (let x = 0; x < viewportWidth; x += 64) {
      grid.lineBetween(x, 0, x, viewportHeight);
    }
    for (let y = 0; y < viewportHeight; y += 64) {
      grid.lineBetween(0, y, viewportWidth, y);
    }
    this.staticLayer.add(grid);

    const locationAnchors = this.buildLocationAnchors();

    // Draw adjacency lines between nearby locations
    const lines = this.add.graphics();
    lines.lineStyle(2, 0x334155, 0.5);
    for (let i = 0; i < locationAnchors.length; i++) {
      for (let j = i + 1; j < locationAnchors.length; j++) {
        const a = locationAnchors[i];
        const b = locationAnchors[j];
        const dist = Phaser.Math.Distance.Between(a.x, a.y, b.x, b.y);
        if (dist < 200) {
          lines.lineBetween(a.x, a.y, b.x, b.y);
        }
      }
    }
    this.staticLayer.add(lines);

    // Draw location tiles
    for (const { location, x, y } of locationAnchors) {
      const region = regionForLocation(location);
      const colors = REGION_COLORS[region] || REGION_COLORS.default;

      // Tile background
      const tile = this.add.rectangle(x, y, TILE_W, TILE_H, colors.bg);
      tile.setStrokeStyle(3, colors.border);
      this.staticLayer.add(tile);

      // Location name — prominent, readable
      const nameText = this.add.text(x, y - 14, location.name, {
        color: "#e2e8f0",
        fontSize: "13px",
        fontFamily: "Inter, Arial, sans-serif",
        fontStyle: "bold",
        align: "center",
        wordWrap: { width: TILE_W - 12 },
      });
      nameText.setOrigin(0.5, 0.5);
      this.staticLayer.add(nameText);

      // Region tag — small, muted
      if (colors.label) {
        const tag = this.add.text(x, y + 16, colors.label, {
          color: `#${colors.border.toString(16).padStart(6, "0")}`,
          fontSize: "9px",
          fontFamily: "Inter, Arial, sans-serif",
          align: "center",
        });
        tag.setOrigin(0.5, 0.5);
        this.staticLayer.add(tag);
      }
    }

    const agentsByLocation = this.buildAgentsByLocation();
    this.renderAgentMarkers(locationAnchors, agentsByLocation);
  }

  private renderAgentMarkers(
    locationAnchors: LocationAnchor[],
    agentsByLocation: Map<string, Agent[]>,
  ) {
    const seenAgents = new Set<string>();

    for (const { location, x, y } of locationAnchors) {
      const locationAgents = agentsByLocation.get(location.id) || [];
      locationAgents.forEach((agent, index) => {
        const offsetX = -22 + (index % 3) * 22;
        const offsetY = TILE_H / 2 + 14 + (index >= 3 ? 24 : 0);
        seenAgents.add(agent.id);
        this.upsertAgentMarker(agent, x + offsetX, y + offsetY);
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

      const circle = this.add.circle(0, 0, 12, colorForAgent(agent.id));
      circle.setStrokeStyle(2, 0x0f172a);
      marker.add(circle);

      const initials = this.add.text(0, 0, getInitials(agent.name), {
        color: "#ffffff",
        fontSize: "10px",
        fontFamily: "Inter, Arial, sans-serif",
        fontStyle: "bold",
      });
      initials.setOrigin(0.5, 0.5);
      marker.add(initials);

      // State indicator
      const stateText = this.add.text(0, -18, stateIcon(agent.state), {
        fontSize: "10px",
      });
      stateText.setOrigin(0.5, 0.5);
      marker.add(stateText);

      // Click handler
      circle.setInteractive({ useHandCursor: true });
      circle.on("pointerdown", () => {
        this.onAgentClick?.(agent.id);
      });

      this.markerLayer.add(marker);
      this.agentMarkers.set(agent.id, marker);
      return;
    }

    // Update selection highlight
    const circle = marker.getAt(0) as Phaser.GameObjects.Arc;
    if (circle && circle.setStrokeStyle) {
      circle.setStrokeStyle(isSelected ? 4 : 2, isSelected ? 0xfbbf24 : 0x0f172a);
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

  private clearAgentMarkers() {
    for (const marker of this.agentMarkers.values()) {
      this.tweens.killTweensOf(marker);
      marker.destroy(true);
    }
    this.agentMarkers.clear();
  }
}
