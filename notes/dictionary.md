# Parsing localized data from YouTube

Since YouTube's API is outputting the website as it should be rendered by the client,
the data received from the API is already localized. This affects dates, times and
number formats.

To be able to successfully parse them, we need to collect samples in every language and
build a dictionary.

### Timeago

- Relative date format used for video upload dates and comments.
- Examples: "1 hour ago", "3 months ago"

### Playlist dates

- Playlist update dates are always day-accurate, either as textual dates or in the form
  of "n days ago"
- Examples: "Last updated on Jan 3, 2020", "Updated today", "Updated yesterday",
  "Updated 3 days ago"

### Video duration

- In Danisch ("da") video durations are formatted using dots instead of colons. Example:
  "12.31", "3.03.52"

### Numbers

- Large numbers (subscriber/view counts) are rounded and shown using a decimal prefix
- Examples: "1.4M views"
- There is an exception for the value 0 ("no views") and in some languages for the value
  1 (pt: "Um vídeo")
- Special case: Language "gu", "જોવાયાની સંખ્યા" = "no views", contains no unique tokens
  to parse
