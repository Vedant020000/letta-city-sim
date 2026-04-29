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

const PADDING_X = 140;
const PADDING_Y = 110;
const GRID_SIZE = 96;
const TILE_SIZE = 76;
const MARKER_TWEEN_MS = 700;

function colorForAgent(agentId: string) {
  const palette = [0x2563eb, 0xdc2626, 0x16a34a, 0x9333ea, 0xea580c, 0x0891b2];
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

export class TownScene extends Phaser.Scene {
  private snapshot: TownSceneSnapshot = { agents: [], locations: [] };
  private worldLayer!: Phaser.GameObjects.Container;
  private staticLayer!: Phaser.GameObjects.Container;
  private markerLayer!: Phaser.GameObjects.Container;
  private agentMarkers = new Map<string, Phaser.GameObjects.Container>();

  constructor() {
    super("TownScene");
  }

  create() {
    this.cameras.main.setBackgroundColor("#9fd3ff");
    this.worldLayer = this.add.container(0, 0);
    this.staticLayer = this.add.container(0, 0);
    this.markerLayer = this.add.container(0, 0);
    // Keep markers on their own layer so websocket redraws can animate them
    // without destroying the active tween every time the static map repaints.
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

  private renderSnapshot() {
    if (!this.staticLayer || !this.markerLayer) return;
    this.staticLayer.removeAll(true);

    const { locations, agents } = this.snapshot;
    if (locations.length === 0) {
      this.clearAgentMarkers();
      return;
    }

    const minX = Math.min(...locations.map((location) => location.map_x));
    const maxX = Math.max(...locations.map((location) => location.map_x));
    const minY = Math.min(...locations.map((location) => location.map_y));
    const maxY = Math.max(...locations.map((location) => location.map_y));

    const viewportWidth = this.scale.width || this.game.canvas.width || 960;
    const viewportHeight = this.scale.height || this.game.canvas.height || 620;
    const spreadX = Math.max(maxX - minX, 1);
    const spreadY = Math.max(maxY - minY, 1);
    const availableWidth = Math.max(viewportWidth - PADDING_X * 2, 1);
    const availableHeight = Math.max(viewportHeight - PADDING_Y * 2, 1);
    const mapScale = Math.min(
      availableWidth / spreadX,
      availableHeight / spreadY,
      1,
    );

    const width = viewportWidth;
    const height = viewportHeight;
    this.cameras.main.setBounds(0, 0, width, height);

    const background = this.add.rectangle(width / 2, height / 2, width, height, 0x93c5fd);
    background.setStrokeStyle(4, 0x60a5fa);
    this.staticLayer.add(background);

    const grid = this.add.graphics();
    grid.lineStyle(1, 0x7dd3fc, 0.4);
    for (let x = 0; x < width; x += GRID_SIZE) {
      grid.lineBetween(x, 0, x, height);
    }
    for (let y = 0; y < height; y += GRID_SIZE) {
      grid.lineBetween(0, y, width, y);
    }
    this.staticLayer.add(grid);

    const agentsByLocation = new Map<string, Agent[]>();
    for (const agent of agents) {
      const existing = agentsByLocation.get(agent.current_location_id) || [];
      existing.push(agent);
      agentsByLocation.set(agent.current_location_id, existing);
    }

    const locationAnchors: LocationAnchor[] = [];
    for (const location of locations) {
      const x = (location.map_x - minX) * mapScale + PADDING_X;
      const y = (location.map_y - minY) * mapScale + PADDING_Y;
      locationAnchors.push({ location, x, y });

      const tile = this.add.rectangle(x, y, TILE_SIZE, TILE_SIZE, 0xf8fafc);
      tile.setStrokeStyle(4, 0x334155);
      this.staticLayer.add(tile);

      const title = this.add.text(x, y - 22, location.name, {
        color: "#0f172a",
        fontSize: "10px",
        fontFamily: "Arial",
        align: "center",
        wordWrap: { width: TILE_SIZE - 8 },
      });
      title.setOrigin(0.5, 0.5);
      this.staticLayer.add(title);

      const idText = this.add.text(x, y + 30, location.id, {
        color: "#475569",
        fontSize: "7px",
        fontFamily: "Consolas",
        align: "center",
        wordWrap: { width: TILE_SIZE - 8 },
      });
      idText.setOrigin(0.5, 0.5);
      this.staticLayer.add(idText);
    }

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
        const offsetX = -18 + (index % 3) * 18;
        const offsetY = 8 + (index >= 3 ? 18 : 0);
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
    let marker = this.agentMarkers.get(agent.id);
    if (!marker) {
      marker = this.add.container(x, y);

      const circle = this.add.circle(0, 0, 10, colorForAgent(agent.id));
      circle.setStrokeStyle(2, 0x0f172a);
      marker.add(circle);

      const initials = this.add.text(0, 0, getInitials(agent.name), {
        color: "#ffffff",
        fontSize: "9px",
        fontFamily: "Arial",
      });
      initials.setOrigin(0.5, 0.5);
      marker.add(initials);

      this.markerLayer.add(marker);
      this.agentMarkers.set(agent.id, marker);
      return;
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
