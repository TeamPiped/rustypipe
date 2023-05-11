# Channel order

Fields:

- `2:0:string` Channel ID
- `15:0:embedded` Videos tab
- `10:0:embedded` Shorts tab
- `14:0:embedded` Livestreams tab
- `2:0:string`: targetId for YouTube's web framework (`"\n$"` + any UUID)
- `3:1:varint` Sort order (1: Latest, 2: Popular)

Popular videos

```json
{
  "80226972:0:embedded": {
    "2:0:string": "UCXuqSBlHAE6Xw-yeJA0Tunw",
    "3:1:base64": {
      "110:0:embedded": {
        "3:0:embedded": {
          "15:0:embedded": {
            "2:0:string": "\n$6461d7c8-0000-2040-87aa-089e0827e420",
            "3:1:varint": 2
          }
        }
      }
    }
  }
}
```

Popular shorts
```json
{
  "80226972:0:embedded": {
    "2:0:string": "UCXuqSBlHAE6Xw-yeJA0Tunw",
    "3:1:base64": {
      "110:0:embedded": {
        "3:0:embedded": {
          "10:0:embedded": {
            "2:0:string": "\n$64679ffb-0000-26b3-a1bd-582429d2c794",
            "3:1:varint": 2
          }
        }
      }
    }
  }
}
```

Popular streams

```json
{
  "80226972:0:embedded": {
    "2:0:string": "UCXuqSBlHAE6Xw-yeJA0Tunw",
    "3:1:base64": {
      "110:0:embedded": {
        "3:0:embedded": {
          "14:0:embedded": {
            "2:0:string": "\n$64693069-0000-2a1e-8c7d-582429bd5ba8",
            "3:1:varint": 2
          }
        }
      }
    }
  }
}
```
