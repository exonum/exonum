var data = [
  {
    "id": 140735236022272,
    "name": "main",
    "spans": [
      {
        "name": "prep",
        "start_ns": 20221,
        "end_ns": 283714883,
        "delta": 283694662,
        "depth": 0,
        "children": [
          {
            "name": "render",
            "start_ns": 44497,
            "end_ns": 283650225,
            "delta": 283605728,
            "depth": 1,
            "children": [
              {
                "name": "collect sampling points",
                "start_ns": 45207,
                "end_ns": 109381383,
                "delta": 109336176,
                "depth": 2,
                "children": [
                  {
                    "name": "build poor mans quad tree",
                    "start_ns": 50929,
                    "end_ns": 52648,
                    "delta": 1719,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "build bitmap",
                    "start_ns": 52958,
                    "end_ns": 50590891,
                    "delta": 50537933,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "filter sample with bitmap",
                    "start_ns": 50592280,
                    "end_ns": 105163082,
                    "delta": 54570802,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "filter points",
                    "start_ns": 105164537,
                    "end_ns": 109352874,
                    "delta": 4188337,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  }
                ],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "gather lines",
                "start_ns": 109382099,
                "end_ns": 238550130,
                "delta": 129168031,
                "depth": 2,
                "children": [
                  {
                    "name": "sampling",
                    "start_ns": 109389870,
                    "end_ns": 238545278,
                    "delta": 129155408,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  }
                ],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "connect lines",
                "start_ns": 238550992,
                "end_ns": 283646172,
                "delta": 45095180,
                "depth": 2,
                "children": [],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "transform lines",
                "start_ns": 283647619,
                "end_ns": 283649907,
                "delta": 2288,
                "depth": 2,
                "children": [],
                "notes": [],
                "collapsable": false
              }
            ],
            "notes": [],
            "collapsable": false
          }
        ],
        "notes": [],
        "collapsable": false
      },
      {
        "name": "real deal",
        "start_ns": 283753847,
        "end_ns": 2059863111,
        "delta": 1776109264,
        "depth": 0,
        "children": [
          {
            "name": "render",
            "start_ns": 283780649,
            "end_ns": 2059844103,
            "delta": 1776063454,
            "depth": 1,
            "children": [
              {
                "name": "collect sampling points",
                "start_ns": 283781106,
                "end_ns": 817573949,
                "delta": 533792843,
                "depth": 2,
                "children": [
                  {
                    "name": "build poor mans quad tree",
                    "start_ns": 283790550,
                    "end_ns": 283791228,
                    "delta": 678,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "build bitmap",
                    "start_ns": 283791495,
                    "end_ns": 767414106,
                    "delta": 483622611,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "filter sample with bitmap",
                    "start_ns": 767415427,
                    "end_ns": 813679255,
                    "delta": 46263828,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  },
                  {
                    "name": "filter points",
                    "start_ns": 813680509,
                    "end_ns": 817553419,
                    "delta": 3872910,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  }
                ],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "gather lines",
                "start_ns": 817574276,
                "end_ns": 2017137714,
                "delta": 1199563438,
                "depth": 2,
                "children": [
                  {
                    "name": "sampling",
                    "start_ns": 817576655,
                    "end_ns": 2017134278,
                    "delta": 1199557623,
                    "depth": 3,
                    "children": [],
                    "notes": [],
                    "collapsable": false
                  }
                ],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "connect lines",
                "start_ns": 2017138518,
                "end_ns": 2059841748,
                "delta": 42703230,
                "depth": 2,
                "children": [],
                "notes": [],
                "collapsable": false
              },
              {
                "name": "transform lines",
                "start_ns": 2059842373,
                "end_ns": 2059843780,
                "delta": 1407,
                "depth": 2,
                "children": [],
                "notes": [],
                "collapsable": false
              }
            ],
            "notes": [],
            "collapsable": false
          }
        ],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145317068800,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1186,
        "end_ns": 44340559,
        "delta": 44339373,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1536,
        "end_ns": 45470760,
        "delta": 45469224,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145312849920,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1340,
        "end_ns": 45679730,
        "delta": 45678390,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145319178240,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1535,
        "end_ns": 46938485,
        "delta": 46936950,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": []
  },
  {
    "id": 123145314959360,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1463,
        "end_ns": 47705679,
        "delta": 47704216,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145317068800,
    "name": null,
    "spans": []
  },
  {
    "id": 123145306521600,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 2489,
        "end_ns": 49776378,
        "delta": 49773889,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145304412160,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 1545,
        "end_ns": 50311135,
        "delta": 50309590,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145306521600,
    "name": null,
    "spans": []
  },
  {
    "id": 123145306521600,
    "name": null,
    "spans": []
  },
  {
    "id": 123145310740480,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 4371,
        "end_ns": 43693936,
        "delta": 43689565,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145312849920,
    "name": null,
    "spans": []
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": []
  },
  {
    "id": 123145319178240,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 715693312,
        "end_ns": 761033710,
        "delta": 45340398,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145314959360,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 716273089,
        "end_ns": 761637379,
        "delta": 45364290,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 716807603,
        "end_ns": 762249160,
        "delta": 45441557,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145306521600,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 716880484,
        "end_ns": 762277290,
        "delta": 45396806,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145304412160,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 716086913,
        "end_ns": 761602500,
        "delta": 45515587,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": []
  },
  {
    "id": 123145317068800,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 716456958,
        "end_ns": 762206964,
        "delta": 45750006,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145317068800,
    "name": null,
    "spans": []
  },
  {
    "id": 123145312849920,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 715903985,
        "end_ns": 761718015,
        "delta": 45814030,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145310740480,
    "name": null,
    "spans": [
      {
        "name": "real sample with bitmap",
        "start_ns": 706459665,
        "end_ns": 752277450,
        "delta": 45817785,
        "depth": 0,
        "children": [],
        "notes": [],
        "collapsable": false
      }
    ]
  },
  {
    "id": 123145314959360,
    "name": null,
    "spans": []
  },
  {
    "id": 123145317068800,
    "name": null,
    "spans": []
  },
  {
    "id": 123145319178240,
    "name": null,
    "spans": []
  },
  {
    "id": 123145308631040,
    "name": null,
    "spans": []
  }
]
