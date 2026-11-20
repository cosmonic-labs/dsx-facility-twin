# Scenario QA report

## CDU pump bearing wear — `CDU-P1-R2-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 | helios:yellow, corvid:yellow, lumen:yellow, meridian:yellow, atlas:yellow, kestrel:yellow | CH-1.head_pressure forecast 55m; CH-2.head_pressure forecast 55m |
| Bearing wear | warn | yellow | 0 |  | CDU-P1-R2-A.pump_vibration forecast 80m |
| Flow loss | escalate | orange | 160 | lumen:yellow, corvid:yellow |  |
| Pump trip | fail | red | 160 | lumen:orange, corvid:orange | P1-R2-02.liquid_return_temp forecast 110m; P1-R2-03.liquid_return_temp forecast 120m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Rack quick-disconnect leak — `P1-R1-03`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Seep at quick-disconnect | warn | yellow | 0 | helios:yellow | P1-R1-03.liquid_flow forecast 95m |
| Leak detected, rack isolated | fail | red | 19 | helios:red | CDU-P1-R1-A.reservoir_level forecast 25m |
| Restore | restore | green | 0 |  | P1-R1-03.liquid_return_temp peer_outlier 0m |
| after restore | - | green | 0 |  |  |

## CDU secondary loop leak — `CDU-P2-R1-B`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Reservoir dropping | warn | yellow | 0 |  | CDU-P2-R1-B.reservoir_level forecast 55m |
| Leak detected, CDU isolated | fail | red | 160 | meridian:orange, helios:orange, kestrel:orange | CDU-P2-R1-A.cdu_supply_temp forecast 5m; CDU-P2-R1-B.cdu_supply_temp forecast 5m; P2-R1-04.liquid_return_temp forecast 5m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Trim chiller trip — `CH-1`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| High head pressure | warn | yellow | 0 |  |  |
| Chiller trips | fail | red | 0 |  |  |
| Standby chiller trips | escalate | red | 644 | helios:orange, corvid:orange, lumen:orange, meridian:orange, atlas:orange, kestrel:orange | P1-R1-01.inlet_air_temp forecast 15m; P1-R1-02.inlet_air_temp forecast 15m; P1-R1-03.inlet_air_temp forecast 15m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Heat wave and dry cooler failure — `DC-2`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Heat building | warn | green | 0 |  |  |
| Dry cooler fan fault | fail | red | 0 | lumen:yellow, corvid:yellow | CDU-P1-R2-A.pump_vibration forecast 80m; CDU-P1-R2-A.cdu_flow forecast 95m; P1-R2-01.liquid_flow forecast 100m |
| Second dry cooler fault | escalate | red | 649 | helios:orange, corvid:orange, lumen:orange, meridian:orange, atlas:orange, kestrel:orange | CDU-P1-R2-A.cdu_flow forecast 40m; P1-R2-01.liquid_flow forecast 40m; P1-R2-02.liquid_flow forecast 40m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Facility water pump trip — `FWP-1`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Pump vibration rising | warn | yellow | 0 | helios:yellow | FWP-1.fwp_vibration forecast 35m; P1-R1-03.liquid_flow forecast 100m |
| Lead pump trips | fail | red | 0 | helios:yellow | P1-R1-03.liquid_flow forecast 40m; CDU-P1-R1-A.reservoir_level forecast 115m |
| Standby drive fault | escalate | red | 649 | helios:red, corvid:red, lumen:red, meridian:red, atlas:red, kestrel:red | P1-R1-03.liquid_flow forecast 40m; CDU-P1-R1-A.reservoir_level forecast 115m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## CRAH failures — `CRAH-2`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| CRAH fan failure | fail | red | 0 |  |  |
| Second CRAH failure | escalate | red | 640 | helios:orange, corvid:orange, lumen:orange, meridian:orange, atlas:orange, kestrel:orange | CDU-P2-R1-B.reservoir_level forecast 115m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Utility outage (generators work) — `UTIL-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Utility sag | warn | yellow | 0 |  |  |
| Utility lost | fail | red | 0 |  |  |
| Running on generators | mitigate | red | 0 |  |  |
| Utility restored | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Utility outage, generator fails to start — `GEN-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Utility lost, generator fails | fail | green | 0 |  |  |
| Batteries depleted | escalate | green | 0 |  |  |
| Utility restored | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## UPS module failure — `UPS-B2`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Module alarm | warn | yellow | 0 |  | UPS-B2.runtime_min runtime 30m |
| Module failed | fail | red | 0 |  |  |
| Second module fails: static bypass | escalate | red | 37 |  |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 | helios:yellow, corvid:yellow, lumen:yellow, meridian:yellow, atlas:yellow, kestrel:yellow | DC-1.approach_temp forecast 80m; DC-2.approach_temp forecast 80m; DC-3.approach_temp forecast 80m |

## Busway tap overload and trip — `P2-R2-02-TAP-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 | helios:yellow, corvid:yellow, lumen:yellow, meridian:yellow, atlas:yellow, kestrel:yellow | DC-1.approach_temp forecast 20m; DC-2.approach_temp forecast 20m; DC-3.approach_temp forecast 20m |
| Tap overload | warn | red | 0 | helios:yellow | P1-R1-03.liquid_flow forecast 5m; P2-R2-02-TAP-A.current forecast 25m |
| Breaker trips | fail | red | 20 | atlas:yellow | P2-R2-02-TAP-B.current forecast 60m |
| Breaker reset | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Busway fault — `BW-P1-R1-B`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Busway de-energized | fail | red | 168 | helios:yellow, corvid:yellow | P1-R1-06-TAP-A.current forecast 30m; P1-R1-02-TAP-A.current forecast 35m; P1-R1-04-TAP-A.current forecast 35m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Transformer overheat and trip — `XFMR-B`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Winding temperature rising | warn | orange | 40 |  | XFMR-B.winding_temp forecast 75m |
| Transformer trips | fail | red | 120 | helios:yellow, lumen:yellow | P1-R2-01-TAP-A.current forecast 115m; P1-R1-03-TAP-A.current forecast 120m; P1-R1-05-TAP-A.current forecast 120m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## DSX Flex grid event — `GRID`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Event announced | warn | yellow | 2 |  |  |
| Power cap active | fail | orange | 2 | atlas:orange |  |
| Event extended | escalate | orange | 2 | corvid:orange, meridian:orange, atlas:orange, kestrel:orange |  |
| Event ends | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## BESS thermal event — `BESS-1`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Cell temperature rising | warn | orange | 0 |  | BESS-1.bess_cell_temp forecast 25m |
| BESS isolated | fail | red | 0 |  |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Rack power shelf failures — `P1-R2-02`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Power shelf fault (1 of 6) | warn | yellow | 19 | lumen:yellow |  |
| Second power shelf fault | fail | orange | 19 | lumen:orange |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Leaf switch failure — `LEAF-P2-R2-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| CRC errors rising | warn | yellow | 0 |  | LEAF-P2-R2-A.link_errors forecast 75m |
| Leaf down | fail | red | 0 |  |  |
| Partner leaf down | escalate | red | 144 | atlas:red |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## NVSwitch tray failure — `P1-R1-05`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| NVLink errors | warn | green | 0 | helios:yellow |  |
| NVSwitch tray failed | fail | green | 0 | helios:orange |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## GPU XID and tray offline — `P1-R1-02-T07`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| ECC / thermal warning | warn | yellow | 0 | helios:yellow |  |
| Tray offline (XID 79) | fail | red | 0 | helios:red |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Drifting, then stuck sensor — `P2-R1-04`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Sensor drifting | warn | green | 0 |  |  |
| Sensor stuck | fail | green | 0 |  | P2-R1-04.liquid_return_temp peer_outlier 0m |
| Sensor replaced | restore | green | 0 |  | P2-R1-04.liquid_return_temp peer_outlier 0m |
| after restore | - | green | 0 |  |  |

## BMS gateway comms loss — `P2-R2`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Gateway retries | warn | green | 0 |  |  |
| Comms lost | fail | green | 0 |  |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

