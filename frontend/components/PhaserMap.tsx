"use client";

import { useEffect, useRef } from "react";
import { Agent, Location } from "@/types/world";

type Props = {
  agents: Agent[];
  locations: Location[];
};

export function PhaserMap({ agents, locations }: Props) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const gameRef = useRef<unknown>(null);
  const sceneRef = useRef<{ applySnapshot: (snapshot: { agents: Agent[]; locations: Location[] }) => void } | null>(null);
  const snapshotRef = useRef({ agents, locations });

  snapshotRef.current = { agents, locations };

  useEffect(() => {
    let cancelled = false;

    async function mountGame() {
      if (!mountRef.current || gameRef.current) {
        return;
      }

      const [Phaser, { TownScene }] = await Promise.all([
        import("phaser"),
        import("@/game/TownScene"),
      ]);

      if (cancelled || !mountRef.current) {
        return;
      }

      const scene = new TownScene();
      sceneRef.current = scene;

      const game = new Phaser.Game({
        type: Phaser.AUTO,
        parent: mountRef.current,
        width: mountRef.current.clientWidth || 960,
        height: mountRef.current.clientHeight || 620,
        backgroundColor: "#93c5fd",
        scene,
        scale: {
          mode: Phaser.Scale.RESIZE,
          autoCenter: Phaser.Scale.CENTER_BOTH,
        },
        render: {
          pixelArt: true,
          antialias: false,
        },
      });

      gameRef.current = game;
      scene.applySnapshot(snapshotRef.current);
    }

    mountGame();

    return () => {
      cancelled = true;
      if (gameRef.current && typeof gameRef.current === "object" && "destroy" in gameRef.current) {
        const game = gameRef.current as { destroy: (removeCanvas?: boolean) => void };
        game.destroy(true);
      }
      gameRef.current = null;
      sceneRef.current = null;
    };
  }, []);

  useEffect(() => {
    sceneRef.current?.applySnapshot(snapshotRef.current);
  }, [agents, locations]);

  return <div ref={mountRef} />;
}
