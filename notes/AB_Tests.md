# A/B-Tests

When YouTube introduces a new feature, it does so gradually. When a user creates a new
session, YouTube decided randomly which new features should be enabled.

YouTube sessions are identified by the visitor data cookie. This cookie is sent with every
API request using the `context.client.visitor_data` JSON parameter. It is also returned in the
`responseContext.visitorData` response parameter and stored as the `__SECURE-YEC` cookie.

By sending the same visitor data cookie, A/B tests can be reproduced, which is important for testing
alternative YouTube clients.

This page lists all A/B tests that were encountered while maintaining the RustyPipe client.

**Impact rating:**

The impact ratings shows how much effort it takes to adapt alternative YouTube clients to the
new feature.

- 🟢 **Low** Minor incompatibility (e.g. parameter name change)
- 🟡 **Medium** Extensive changes to the response data model OR removal of parameters
- 🔴 **High** Changes to the functionality of YouTube that will require API changes
  for alternative clients

If you want to check how often these A/B tests occur, you can use the `codegen` tool with the
following command: `rustypipe-codegen ab-test <id>`.

## [1] Attributed text description

- **Encountered on:** 24.09.2022
- **Impact:** 🟡 Medium
- **Endpoint:** next (video details)

![A/B test 1 screenshot](./_img/ab_1.png)

YouTube shows internal links (channels, videos, playlists) in the video description
as buttons with the YouTube icon. To accomplish this, they completely changed the underlying
data model.

The new format uses a string with the entire plaintext content along with a list of `"commandRuns"`
which include the link data and the position of the links within the text.

Note that the position and length parameter refer to the number of UTF-16 characters. If
you are implementing this in a language which does not use UTF-16 as its internal string
representation, you have to iterate over the unicode codepoints and keep track of the UTF-16
index seperately.

**OLD**

```json
{
  "videoSecondaryInfoRenderer": {
    "description": {
      "runs": [
        {
          "text": "🎧Listen and download aespa's debut single \"Black Mamba\": "
        },
        {
          "navigationEndpoint": {
            "commandMetadata": {
              "webCommandMetadata": {
                "rootVe": 83769,
                "url": "https://www.youtube.com/redirect?...",
                "webPageType": "WEB_PAGE_TYPE_UNKNOWN"
              }
            },
            "urlEndpoint": {
              "nofollow": true,
              "target": "TARGET_NEW_WINDOW",
              "url": "https://www.youtube.com/redirect?..."
            }
          },
          "text": "https://smarturl.it/aespa_BlackMamba"
        }
      ]
    }
  }
}
```

**NEW**

```json
{
  "videoSecondaryInfoRenderer": {
    "attributedDescription": {
      "content": "🎧Listen and download aespa's debut single \"Black Mamba\": https://smarturl.it/aespa_BlackMamba\n🐍The Debut Stage...",
      "commandRuns": [
        {
          "startIndex": 58,
          "length": 36,
          "onTap": {
            "innertubeCommand": {
              "commandMetadata": {
                "webCommandMetadata": {
                  "url": "https://www.youtube.com/redirect?...",
                  "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                  "rootVe": 83769
                }
              },
              "urlEndpoint": {
                "url": "https://www.youtube.com/redirect?...",
                "target": "TARGET_NEW_WINDOW",
                "nofollow": true
              }
            }
          }
        }
      ]
    }
  }
}
```

## [2] 3-tab channel layout

- **Announced:** 15.09.2022, https://www.youtube.com/watch?v=czIyqEC4V-s
- **Encountered on:** 11.10.2022
- **Impact:** 🔴 High
- **Endpoint:** browse (channel videos)

![A/B test 2 screenshot](./_img/ab_2.webp)

YouTube changed their channel page layout, putting livestreams and short videos into
separate tabs.

Fetching the videos page now only returns a subset of a channel's videos. To get all videos
from a channel, you would have to run up to 3 queries.

Even though it has its disadvantages, the RSS feed is now probably the best way for keeping
track of a channel's new uploads.

Additionally the channel tab response model was slightly changed, now using a `"RichGridRenderer"`.
Short videos also have their own data models (`"reelItemRenderer"`).

**RichGrid**

```json
{
  "tabRenderer": {
    "content": {
      "richGridRenderer": {
        "contents": [
          {
            "richItemRenderer": {
              "content": {
                "videoRenderer": {}
              }
            }
          }
        ]
      }
    }
  }
}
```

**Short video**

```json
{
  "reelItemRenderer": {
    "accessibility": {
      "accessibilityData": {
        "label": "being smart was my personality trait - 56 seconds - play video"
      }
    },
    "headline": {
      "simpleText": "being smart was my personality trait"
    },
    "navigationEndpoint": {
      "clickTrackingParams": "CLcCEIf2BBgAIhMImuP85t-D-wIVd-sRCB2r6gl7",
      "commandMetadata": {
        "webCommandMetadata": {
          "rootVe": 37414,
          "url": "/shorts/glyJWxp7a5g",
          "webPageType": "WEB_PAGE_TYPE_SHORTS"
        }
      },
      "reelWatchEndpoint": {
        "overlay": {
          "reelPlayerOverlayRenderer": {
            "reelPlayerHeaderSupportedRenderers": {
              "reelPlayerHeaderRenderer": {
                "timestampText": {
                  "simpleText": "2 days ago"
                }
              }
            }
          }
        }
      }
    },
    "thumbnail": {
      "thumbnails": [
        {
          "height": 720,
          "url": "https://i.ytimg.com/vi/glyJWxp7a5g/hq720_2.jpg?sqp=-oaymwEdCJUDENAFSFXyq4qpAw8IARUAAIhCcAHAAQbQAQE=&rs=AOn4CLCUzo9AlrNh4n4cZfTOB8_Gf5aAkw",
          "width": 405
        }
      ]
    },
    "videoId": "glyJWxp7a5g",
    "viewCountText": {
      "simpleText": "593K views"
    }
  }
}
```

## [3] Channel handles in search results

- **Encountered on:** 20.11.2022
- **Impact:** 🟡 Medium
- **Endpoint:** search

![A/B test 3 screenshot](./_img/ab_3.png)

Instead of subscriber count / video count, a channel item from the search result now
displays the channel handle and the subscriber count. The video count was removed.

The implementation looks pretty quick and dirty, as they did not even bother to rename
their response parameters. So this might change again in the future.

Note that channels without handles still use the old data model, even on the same page.

**OLD**

```json
{
  "subscriberCountText": {
    "accessibility": {
      "accessibilityData": {
        "label": "2.92 million subscribers"
      }
    },
    "simpleText": "2.92M subscribers"
  },
  "videoCountText": {
    "runs": [
      {
        "text": "219"
      },
      {
        "text": " videos"
      }
    ]
  }
}
```

**NEW**

```json
{
  "videoCountText": {
    "accessibility": {
      "accessibilityData": {
        "label": "4.03 million subscribers"
      }
    },
    "simpleText": "4.03M subscribers"
  },
  "subscriberCountText": {
    "simpleText": "@MusicTravelLove"
  }
}
```

## [4] Video tab on the Trending page

- **Encountered on:** 1.04.2023
- **Impact:** 🟢 Low
- **Endpoint:** browse (trending videos)

YouTube moved the list of trending videos from the main *trending* page to a
separate tab (Videos).

The video tab is fetched with the params `4gIOGgxtb3N0X3BvcHVsYXI%3D`.

This new tab contains two shelves (video lists), the first one labeled "Trending videos"
which contains the regular trends and the second one named "Recently trending".

The data model for the video shelves did not change.

**OLD**

![A/B test 4 old screenshot](./_img/ab_4_old.png)

**NEW**

![A/B test 4 old screenshot](./_img/ab_4_new.png)

## [5] Page header renderer on the Trending page

- **Encountered on:** 1.05.2023
- **Impact:** 🟢 Low
- **Endpoint:** browse (trending videos)

YouTube changed the header renderer type on the trending page to a `pageHeaderRenderer`.

**OLD**

```json
"c4TabbedHeaderRenderer": {
  "avatar": {
    "thumbnails": [
      {
        "height": 100,
        "url": "https://www.youtube.com/img/trending/avatar/trending_avatar.png",
        "width": 100
      }
    ]
  },
  "title": "Trending",
  "trackingParams": "CBAQ8DsiEwiXi_iUht76AhVM6hEIHfgTB2g="
}
```

**NEW**

```json
"pageHeaderRenderer": {
  "pageTitle": "Trending",
  "content": {
    "pageHeaderViewModel": {
      "title": {
        "dynamicTextViewModel": { "text": { "content": "Trending" } }
      },
      "image": {
        "contentPreviewImageViewModel": {
          "image": {
            "sources": [
              {
                "url": "https://www.youtube.com/img/trending/avatar/trending.png",
                "width": 100,
                "height": 100
              }
            ]
          },
          "style": "CONTENT_PREVIEW_IMAGE_STYLE_CIRCLE"
        }
      }
    }
  }
}
```
