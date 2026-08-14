# Getting the preacher's voice into the app

The app listens to one audio input and finds the scripture being preached. **Which
input you give it matters more than any setting inside the app.**

## Why the cable beats the microphone

The laptop's own microphone hears the *room*: the loudspeakers, the reverb off the
walls, the congregation, the fan in the projector. A feed from the sound desk carries
the preacher's own microphone, already mixed, with none of that in it.

This is not a small difference. The app's accuracy was measured against eight recorded
sermons — all of them desk recordings — and it found **81%** of the verses that were
projected that day. Nothing in those recordings tells us how it copes with a laptop
microphone at the back of a hall, and it will certainly cope worse. Use the desk if you
possibly can.

## How to connect, in order of preference

**1. The mixer has a USB port (most desks made in the last decade).**
One USB cable from the mixer to the laptop. The mixer appears in the app's *Sound
input* list under its own name. Nothing to buy.

**2. The mixer has outputs but no USB.**
Take any spare output — a spare aux send, the tape/RCA out, the monitor out, or a
"main out" if there's a second one — into a small USB audio interface (a Behringer
UCA202 is about $30; any class-compliant interface works). The interface then appears
in the *Sound input* list. This works with **any** desk, however old.

*Don't take the only main output that feeds the speakers.* Use a spare send.

**3. The mixer's output straight into the laptop's 3.5 mm jack.**
Cheapest, but most laptops now have a single headset jack expecting *microphone*
level, and a desk output is far hotter than that — it will distort. If you must do
this, turn the send right down and use **Test sound** (below) to check for clipping.

**4. A recorder already in the signal path** (Zoom, Tascam, etc.).
Many can run as a USB interface — plug it into the laptop and it shows up as an input.

**5. Last resort: the laptop's own microphone.**
Put it as close to a speaker as you can and expect it to miss more.

## The laptop's own microphone, and when to use it

It is in the *Sound input* list, in its own group headed **"Demonstration only — hears
the room, not the preacher"**. It is there for one job: showing somebody what the app
does. No cable, no mixer — open the app, pick it, talk, and watch the verse you quoted
come up on the screen.

Choosing it takes a second press to confirm, it is never selected by default, and while
it is in use the app says **"demonstration input — hears the room"** where the operator
can see it without going looking. That is deliberate. Listening to the wrong thing is
worse than not listening at all, because from the operator's chair it looks exactly like
it is working — so the app will do it, but never quietly.

For a service, use the desk.

## Before the service: press "Test sound"

In *Live listening* → *Sound input*, pick the device and press **Test sound**. It
listens for three seconds and tells you what is arriving:

| It says | Do this |
|---|---|
| sound is arriving | You're ready. |
| nothing arriving | The cable, the send, or the mute button. The app is deaf right now. |
| very quiet | Turn the send up at the desk. Quiet audio makes the recognizer guess. |
| clipping | Turn the send down. Distortion is as bad as silence. |

If the input is unplugged mid-service, the app says so out loud rather than quietly
switching to the laptop microphone and pretending to listen. There is no fallback of any
kind: the laptop microphone is only ever used when it was picked on purpose.

## What the app copes with on its own

- **Any sample format** the hardware presents — 16-bit, 24-bit (arriving as 32-bit),
  float, unsigned. Older interfaces are not all the same and none of them are refused.
- **Any sample rate** (44.1 kHz, 48 kHz, whatever) — resampled to what the recognizer
  wants, with proper averaging rather than crude sample-dropping, which would smear
  the consonants it relies on.
- **Half-patched stereo.** If only the left channel is connected — which is normal on
  a desk feed — the dead channel is left out rather than averaged in and costing you
  6 dB of level.
- **Quiet feeds**, up to a point: levels are normalised before recognition.

## Tuning a speaker

*Voice calibration* stores settings **per speaker** (Miss Hilda, the Vice-President, a
guest). Calibrating one never disturbs another.

Calibrate **through the input the service will actually use**. Tuning a preacher on the
laptop microphone at the desk, when the service will run through the mixer feed, tunes
the app for a sound it will never hear.
