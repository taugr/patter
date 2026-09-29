# Calendar and meetings

Patter reads the calendars already connected to your Mac. Google Calendar works through macOS Calendar; it does not need a separate Google Cloud project or Patter Calendar OAuth client.

## Connect Google Calendar

1. Open macOS **System Settings → Internet Accounts**, add your Google account and enable **Calendars**.
2. Open Apple Calendar and confirm that your meetings appear there.
3. In Patter, choose **Settings → Calendar → Connect calendar** and allow full Calendar access.

Connection errors appear beside the button. Expand **Connection help** for Calendar permission and Internet Accounts shortcuts. If access was denied, use **Calendar permissions**, enable Patter, return and reconnect. **Check again** reads the current status without requesting access. See the [permission guide](permissions.md) if Patter is missing from macOS settings.

Patter reads events without changing them. Connecting Drive backup is a separate action and does not connect your calendar.

## Open notes or join a call

Select a meeting under **Upcoming** to create or reopen its linked conversation. Patter keeps the event's title, time, calendar and available links with the note. Reopening the same event reuses the existing conversation, including an archived one.

Use **Join** under Upcoming, or **Join meeting** in a linked conversation or reminder, to open a Google Meet or Zoom link. Patter looks in the event's URL, location and invitation notes. Zoom passcodes in the URL are preserved. The link opens in your default browser, which can hand a Zoom call to the Zoom app. Events without a recognised meeting link have no Join button. Joining a call does not start recording.

The sidebar's **Record → Record microphone & computer audio** action starts a new conversation. **Record…** from a meeting reminder asks for confirmation and records into that meeting's linked conversation, reusing it when one already exists.

## Configure reminders

After connecting Calendar, open **Settings → Calendar → Reminders**:

1. Enable **Notify me before meetings** and allow macOS notifications.
2. Choose **Remind me**: at start time, or 1, 2, 5, 10, 15 or 30 minutes before.
3. Choose whether to **Play sound** and **Show meeting titles** in notifications.
4. Expand **Calendars** to limit reminders to selected calendars. An empty selection means all calendars; **Use all calendars** clears the selection.
5. Choose **Save changes**.

Use **Test notification** to check delivery. It does not read a calendar or start recording. Expand **Notification settings** if nothing appears, then check macOS Notifications and Focus. A reminder can open notes or offer recording confirmation; audio never starts automatically.

## When reminders run

Keep Patter open; it can run in the background. Calendar events refresh every minute and reminder timing is checked every ten seconds. Reminders stop when Patter quits and cannot run while the Mac sleeps. After waking, an upcoming meeting or one that started less than a minute ago can still produce a reminder.

All-day, cancelled and declined events are skipped. Each delivered meeting occurrence is remembered across restarts to avoid duplicates; a rescheduled occurrence can notify again. Preferences are included in library backups, but notification permission must be granted on each Mac.

macOS controls banner and sound delivery. Calendar consent has been confirmed in a packaged local test build; real calendar-triggered notifications and their macOS banner actions still need an end-to-end check.
