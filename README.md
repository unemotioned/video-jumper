# Video Jumper

Media player controller CLI written in [Rust](https://rust-lang.org/).

After watching the anime [Nichijou](https://nichijou.fandom.com/wiki/Nichijou)
more than 10 times, I came up with an idea:

> What if I could watch specific episodes from a video without editing the
> original files?

## Table of Contents

- [Features](#features)
- [TODO](#todo)

---

## Features

- work with variety of media players from macOS, Windows and Linux
- play a video or multiple videos in a queue
- control when to jump and where to jump to from terminal
- choose whether to use jump feature through terminal commands with flags or options
- query the current video's frame or time in real time
- list available videos
- edit the config file entirely through CLI commands
- save jump-points for each video in a single config file
- config file contents:
  - video name
  - video path
  - video comment
  - jump-points
  - jump-point comment
- gracefully terminate the process when the video or queue ends
- save current state (video frame/time, queue) on exit
- start from last saved state

---

## TODO

- [x] launch media player and play video
- [ ] add vlc to Windows system environment variable path
- [ ] start with vj command from terminal
- [ ] terminate process when video ends
- [ ] play multiple videos using a queue
- [ ] jump from one frame to another
- [ ] parse arguments
- [ ] edit config file from the terminal
- [ ] query currently playing video's frame/time
