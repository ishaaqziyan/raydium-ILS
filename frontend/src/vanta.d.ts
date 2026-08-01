declare module "vanta/dist/vanta.topology.min" {
  interface VantaTopologyOptions {
    el: string | HTMLElement;
    p5?: unknown;
    THREE?: unknown;
    mouseControls?: boolean;
    touchControls?: boolean;
    gyroControls?: boolean;
    minHeight?: number;
    minWidth?: number;
    scale?: number;
    scaleMobile?: number;
    color?: number;
    backgroundColor?: number;
  }

  interface VantaEffect {
    destroy(): void;
  }

  type VantaTopologyFactory = (options: VantaTopologyOptions) => VantaEffect;

  const factory: VantaTopologyFactory;
  export default factory;
}
