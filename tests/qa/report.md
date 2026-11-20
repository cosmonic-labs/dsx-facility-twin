# Scenario QA report

## CDU pump bearing wear — `CDU-P1-R2-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Bearing wear | warn | yellow | 0 |  | CDU-P1-R2-A.pump_vibration forecast 100m |
| Flow loss | escalate | orange | 160 | lumen:yellow, corvid:yellow | CDU-P1-R2-A.cdu_flow forecast 30m; P1-R2-07.liquid_flow forecast 40m; P1-R2-08.liquid_flow forecast 45m |
| Pump trip | fail | red | 160 | lumen:orange, corvid:orange |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Rack quick-disconnect leak — `P1-R1-03`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Seep at quick-disconnect | warn | yellow | 0 | helios:yellow | CDU-P1-R1-A.reservoir_level forecast 90m |
| Leak detected, rack isolated | fail | red | 0 | helios:red | P1-R1-03.liquid_return_temp peer_outlier 0m; CDU-P1-R1-A.reservoir_level forecast 50m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Heat wave and dry cooler failure — `DC-2`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Heat building | warn | green | 0 | helios:yellow, corvid:yellow, lumen:yellow, meridian:yellow, atlas:yellow, kestrel:yellow | FWS.fw_supply_temp forecast 90m; CDU-P1-R2-A.cdu_supply_temp forecast 90m; CDU-P1-R2-B.cdu_supply_temp forecast 90m |
| Dry cooler fan fault | fail | red | 0 |  |  |
| Second dry cooler fault | escalate | red | 649 | helios:orange, corvid:orange, lumen:orange, meridian:orange, atlas:orange, kestrel:orange | DC-1.approach_temp forecast 100m |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Facility water pump trip — `FWP-1`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Pump vibration rising | warn | yellow | 0 |  | FWP-1.fwp_vibration forecast 15m |
| Lead pump trips | fail | red | 0 |  |  |
| Standby drive fault | escalate | red | 649 | helios:red, corvid:red, lumen:red, meridian:red, atlas:red, kestrel:red |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Busway tap overload and trip — `P2-R2-02-TAP-A`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Tap overload | warn | orange | 0 |  | P2-R2-02-TAP-A.current forecast 5m |
| Breaker trips | fail | red | 0 |  |  |
| Breaker reset | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Transformer overheat and trip — `XFMR-B`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Winding temperature rising | warn | yellow | 40 |  | XFMR-B.winding_temp forecast 35m |
| Transformer trips | fail | red | 40 |  |  |
| Restore | restore | green | 0 |  |  |
| after restore | - | green | 0 |  |  |

## Drifting, then stuck sensor — `P2-R1-04`

| Stage | Kind | Target | Downstream | Customers | Predictions |
|---|---|---|---|---|---|
| baseline | - | green | 0 |  |  |
| Sensor drifting | warn | yellow | 0 | meridian:yellow | P2-R1-04.liquid_return_temp forecast 90m |
| Sensor stuck | fail | yellow | 0 | meridian:yellow | P2-R1-04.liquid_return_temp peer_outlier 0m; P2-R1-04.liquid_return_temp forecast 90m |
| Sensor replaced | restore | green | 0 |  | P2-R1-04.liquid_return_temp peer_outlier 0m |
| after restore | - | green | 0 |  | P2-R1-04.liquid_return_temp peer_outlier 0m |

