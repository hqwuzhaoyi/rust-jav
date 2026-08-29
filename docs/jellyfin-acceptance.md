# Jellyfin integration acceptance

The integration is deliberately an index/refresh integration. rust-jav reads
server information, media folders, item paths/provider IDs/playback state, and
opens the Jellyfin web detail page. It neither streams nor deletes Jellyfin
items. Local filesystem authority and Jellyfin association are separate;
metadata-only matches are shown as uncertain and cannot authorize deletion.

The implementation follows Jellyfin's official stable OpenAPI operations:
`GET /System/Info`, `GET /Library/MediaFolders`, `GET /Items`, and
`POST /Library/Refresh`. Authentication is sent from the rust-jav server as an
`X-Emby-Token` header. Jellyfin's official networking guide identifies TCP
8096 as the default HTTP port and notes that container/VM firewalls form a
separate network layer.

Official sources checked on 2026-08-29:

- <https://api.jellyfin.org/openapi/jellyfin-openapi-stable.json>
- <https://jellyfin.org/docs/general/post-install/networking/>
- <https://jellyfin.org/docs/general/server/libraries/>

## Real TrueNAS SCALE procedure

1. Put the rust-jav and Jellyfin Apps on a network where rust-jav can resolve
   and reach Jellyfin's App DNS name and HTTP port. Do not use `localhost`
   unless both processes share a network namespace.
2. In Jellyfin Dashboard > API Keys, create a key dedicated to rust-jav. Do
   not enter a user password or browser token.
3. Sign into rust-jav, open Settings > Jellyfin, enter the App-network URL,
   comma-separated library IDs, and API key, then save.
4. Select **Test connection**. Confirm the expected Jellyfin server name and
   every selected library are returned. A missing selected ID must fail the
   test rather than silently widening scope.
5. Reconcile a Media Root whose paths match Jellyfin item paths. Inspect an
   asset and confirm its playback status and **Open in Jellyfin** link. Repeat
   with a code/title-only match and confirm it is visibly uncertain.
6. Run one applied Management Task touching multiple files. Confirm Jellyfin
   receives one library refresh request for the batch. Preview tasks must not
   refresh Jellyfin.
7. Stop or disconnect Jellyfin and request a refresh. Confirm refresh state is
   tracked separately, attempts follow bounded exponential backoff, stop at
   five, and show `manual_retry_required`. Restore Jellyfin and use **Refresh
   Jellyfin** to retry manually.
8. Inspect browser responses and logs: the API key must never appear. Verify
   Jellyfin received no item delete, file, download, playback-info, or media
   streaming request during the procedure.
