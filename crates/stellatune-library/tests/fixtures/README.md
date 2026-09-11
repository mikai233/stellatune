`metadata-id3.mp3` is 0.15 seconds of generated silence with ID3v2.3 tags,
used to verify that library scanning enables metadata readers as well as audio codecs.
It contains no user audio. Generated with:

```sh
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 0.15 -c:a libmp3lame -b:a 64k \
  -id3v2_version 3 -metadata title="Tagged Song" -metadata artist="Tagged Artist" \
  -metadata album="Tagged Album" -metadata album_artist="Album Artist" \
  -metadata track="2/10" -metadata disc="1/2" metadata-id3.mp3
```
