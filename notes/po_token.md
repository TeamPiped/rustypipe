# About the new `pot` token

YouTube has implemented a new method to prevent downloaders and alternative clients from accessing
their videos. Now requests to YouTube's video servers require a `pot` URL parameter.

It is currently only required in the web player. The YTM and embedded player sends the token, too, but does not require it (this may change in the future).

The TV player does not use the token at all and is currently the best workaround. The only downside
is that the TV player does not return any video metadata like title and description text.

The first part of a video file (range: 0-1007959 bytes) can be downloaded without the token.
Requesting more of the file requires the pot token to be set, otherwise YouTube responds with a 403
error.

The pot token is base64-formatted and usually starts with a M

`MnToZ2brHmyo0ehfKtK_EWUq60dPYDXksNX_UsaniM_Uj6zbtiIZujCHY02hr7opxB_n3XHetJQCBV9cnNHovuhvDqrjfxsKR-sjn-eIxqv3qOZKphvyDpQzlYBnT2AXK41R-ti6iPonrvlvKIASNmYX2lhsEg==`

The token is generated from YouTubes Botguard script. The token is bound to the visitor data ID
used to fetch the player data.

This feature has been A/B-tested for a few weeks. During that time, refetching the player in case
of a 403 download error often made things work again. As of 08.08.2024 this new feature seems to be
stabilized and retrying requests does not work any more.

## Getting a `pot` token

You need a real browser environment to run YouTube's botguard and obtain a pot token. The Invidious project has created a script to
<https://github.com/iv-org/youtube-trusted-session-generator/tree/master>.
The script opens YouTube's embedded video player, starts playback and extracts the visitor data
