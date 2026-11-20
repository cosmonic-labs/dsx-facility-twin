/** The suggested demo: three scenarios in order of blast radius, each with a clear early warning.
 *  Stage notes are what to point at (and say) while that stage is on screen. */
export interface Story {
  id: string;
  tier: "small" | "medium" | "catastrophic";
  tierLabel: string;
  headline: string;
  pitch: string;
  view: string; // where to watch it
  stages: Record<string, string>;
}

export const STORIES: Story[] = [
  {
    id: "tap-overload-trip",
    tier: "small",
    tierLabel: "Small impact",
    headline: "A tenant outgrows its breaker",
    pitch: "One rack's tap-off current climbs. augurs forecasts the trip before it happens; when the breaker opens, 2N power keeps the customer running. Nobody is affected, but the rack has lost its spare cord.",
    view: "power",
    stages: {
      overload: "Current crosses 80% of the breaker: the tap turns yellow, then orange. Open Warnings: augurs shows the forecast line reaching the trip point.",
      "breaker-trips": "The breaker opens. The tap is red, the rack stays green on its other cord. The twin flags redundancy lost instead of crying outage.",
      restore: "Breaker reset. Everything clears to green.",
    },
  },
  {
    id: "cdu-pump-wear",
    tier: "medium",
    tierLabel: "Medium impact",
    headline: "A CDU pump wears out",
    pitch: "Vibration on a row's CDU pump starts rising long before anything fails. The forecast is the early warning; then flow drops, two customers feel it, and the pump trips.",
    view: "cooling",
    stages: {
      "bearing-wear": "Vibration passes its watch level: CDU yellow, and augurs forecasts when it reaches the degraded limit. No customer impact yet. This is the moment to schedule the repair.",
      "flow-loss": "Flow falls below half. The CDU is orange and the row's racks turn yellow: Lumen and Corvid are on trend to be throttled. Click the CDU in Cooling to see the customer tree under the graph.",
      "pump-trip": "The pump trips. The CDU is red, the partner CDU carries the row alone, and Lumen and Corvid are degraded. Open Customers for the revenue at risk.",
      restore: "Repair done. Flow and temperatures recover on their own time constants.",
    },
  },
  {
    id: "fw-pump-trip",
    tier: "catastrophic",
    tierLabel: "Catastrophic",
    headline: "The hall loses facility water",
    pitch: "A lead facility-water pump vibrates, trips, and the standby takes over. Then the standby's drive faults. Every CDU loses heat rejection: the widest blast radius in the hall, and it all started with one yellow pump.",
    view: "cooling",
    stages: {
      vibration: "Lead pump vibration rising: one yellow pump, and augurs forecasts the trend. Easy to ignore. Remember it.",
      "lead-trips": "The lead pump trips and the standby picks up. Pump red, facility water green but on a single pump: redundancy lost. Still no customer impact.",
      "standby-fault": "The standby faults. Facility water is out, every CDU and rack goes red, and all six customers are affected. Open Impact explorer by customer to walk the tree back to the pump.",
      restore: "Pumps restored. Watch the hall cool back down.",
    },
  },
];

export const storyFor = (id: string) => STORIES.find((s) => s.id === id);
