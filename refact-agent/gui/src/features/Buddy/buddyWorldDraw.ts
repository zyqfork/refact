import type { BuddyWorldState } from "./buddyWorldModel";
import type { Palette } from "./types";
import {
  drawAmbientLayers,
  drawCelestial,
  drawObservatorySky,
  drawSkyGradient,
  drawWeatherAtmosphere,
} from "./buddyWorldDrawAtmosphere";
import {
  drawBuddyLandingPad,
  drawDistantHills,
  drawForegroundCozyDetails,
  drawGround,
  drawHomePath,
  drawMidgroundGarden,
  drawVitality,
  drawWorkshopZones,
} from "./buddyWorldDrawDiorama";
import { drawBuddyHomeDoor, drawWorldObjects } from "./buddyWorldDrawObjects";
import {
  safeDimension,
  safeFrame,
  type DrawBuddyWorldBaseArgs,
} from "./buddyWorldDrawHelpers";

export interface DrawBuddyWorldArgs {
  ctx: CanvasRenderingContext2D;
  world: BuddyWorldState;
  palette: Palette;
  frame: number;
  width: number;
  height: number;
  compact: boolean;
  reducedMotion: boolean;
}

export function drawBuddyWorld(args: DrawBuddyWorldArgs): void {
  const width = safeDimension(args.width, 720);
  const height = safeDimension(args.height, args.compact ? 190 : 260);
  const drawArgs: DrawBuddyWorldBaseArgs = {
    ...args,
    frame: safeFrame(args.frame),
    width,
    height,
  };

  args.ctx.clearRect(0, 0, width, height);
  args.ctx.imageSmoothingEnabled = false;

  drawSkyGradient(drawArgs);
  drawObservatorySky(drawArgs);
  drawCelestial(drawArgs);
  drawAmbientLayers(drawArgs);
  drawDistantHills(drawArgs);
  drawMidgroundGarden(drawArgs);
  drawWorkshopZones(drawArgs);
  drawWeatherAtmosphere(drawArgs);
  drawWorldObjects(drawArgs);
  drawGround(drawArgs);
  drawHomePath(drawArgs);
  drawBuddyHomeDoor(drawArgs);
  drawVitality(drawArgs);
  drawBuddyLandingPad(drawArgs);
  drawForegroundCozyDetails(drawArgs);
}
