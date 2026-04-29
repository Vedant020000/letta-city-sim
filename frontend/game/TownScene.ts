import * as Phaser from "phaser";
import { Agent, Location } from "@/types/world";

type TownSceneSnapshot = {
  agents: Agent[];
  locations: Location[];
};

const PADDING_X = 140;
const PADDING_Y = 110;
const GRID_SIZE = 96;
const TILE_SIZE = 76;

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

  constructor() {
    super("TownScene");
  }

  create() {
    this.cameras.main.setBackgroundColor("#9fd3ff");
    this.worldLayer = this.add.container(0, 0);
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
    if (!this.worldLayer) return;
    this.worldLayer.removeAll(true);

    const { locations, agents } = this.snapshot;
    if (locations.length === 0) {
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
    this.worldLayer.add(background);

    const grid = this.add.graphics();
    grid.lineStyle(1, 0x7dd3fc, 0.4);
    for (let x = 0; x < width; x += GRID_SIZE) {
      grid.lineBetween(x, 0, x, height);
    }
    for (let y = 0; y < height; y += GRID_SIZE) {
      grid.lineBetween(0, y, width, y);
    }
    this.worldLayer.add(grid);

    const agentsByLocation = new Map<string, Agent[]>();
    for (const agent of agents) {
      const existing = agentsByLocation.get(agent.current_location_id) || [];
      existing.push(agent);
      agentsByLocation.set(agent.current_location_id, existing);
    }

    for (const location of locations) {
      const x = (location.map_x - minX) * mapScale + PADDING_X;
      const y = (location.map_y - minY) * mapScale + PADDING_Y;

      const tile = this.add.rectangle(x, y, TILE_SIZE, TILE_SIZE, 0xf8fafc);
      tile.setStrokeStyle(4, 0x334155);
      this.worldLayer.add(tile);

      const title = this.add.text(x, y - 22, location.name, {
        color: "#0f172a",
        fontSize: "10px",
        fontFamily: "Arial",
        align: "center",
        wordWrap: { width: TILE_SIZE - 8 },
      });
      title.setOrigin(0.5, 0.5);
      this.worldLayer.add(title);

      const idText = this.add.text(x, y + 30, location.id, {
        color: "#475569",
        fontSize: "7px",
        fontFamily: "Consolas",
        align: "center",
        wordWrap: { width: TILE_SIZE - 8 },
      });
      idText.setOrigin(0.5, 0.5);
      this.worldLayer.add(idText);

      const locationAgents = agentsByLocation.get(location.id) || [];
      locationAgents.forEach((agent, index) => {
        const offsetX = -18 + (index % 3) * 18;
        const offsetY = 8 + (index >= 3 ? 18 : 0);
        const marker = this.add.circle(x + offsetX, y + offsetY, 10, colorForAgent(agent.id));
        marker.setStrokeStyle(2, 0x0f172a);
        this.worldLayer.add(marker);

        const initials = this.add.text(x + offsetX, y + offsetY, getInitials(agent.name), {
          color: "#ffffff",
          fontSize: "9px",
          fontFamily: "Arial",
        });
        initials.setOrigin(0.5, 0.5);
        this.worldLayer.add(initials);
      });
    }
  }
}
