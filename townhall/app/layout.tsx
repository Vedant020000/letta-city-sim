import type { ReactNode } from "react";
import type { Metadata } from "next";
import { Press_Start_2P, VT323 } from "next/font/google";
import "./globals.css";

const pressStart = Press_Start_2P({
  subsets: ["latin"],
  weight: ["400"],
  variable: "--font-display",
  display: "swap",
});

const vt323 = VT323({
  subsets: ["latin"],
  weight: ["400"],
  variable: "--font-body",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Townhall - letta-city-sim",
  description:
    "Simple pixel-art community board for letta-city-sim. Architecture stays maintainer-owned; community tasks live here.",
  openGraph: {
    title: "Townhall - letta-city-sim",
    description:
      "Browse community-open issues, claim them with GitHub comments, and help build Smallville.",
  },
};

export default function RootLayout({ children }: Readonly<{ children: ReactNode }>) {
  return (
    <html lang="en" className={`${pressStart.variable} ${vt323.variable}`}>
      <body>{children}</body>
    </html>
  );
}
