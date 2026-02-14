function run() {
  const out = {
    state: "stopped",
  };

  const music = Application("Music");
  if (!music.running()) {
    return JSON.stringify(out);
  }

  try {
    const state = String(music.playerState());
    out.state = state;

    if (state === "playing" || state === "paused") {
      const track = music.currentTrack();
      if (track) {
        out.title = track.name();
        out.artist = track.artist();
        out.album = track.album();
        out.duration = Math.round((track.duration() || 0) * 1000);
        out.position = Math.round((music.playerPosition() || 0) * 1000);
        out.persistentId = track.persistentID();
      }
    }

    return JSON.stringify(out);
  } catch (e) {
    return JSON.stringify({ state: "error", error: String(e) });
  }
}
