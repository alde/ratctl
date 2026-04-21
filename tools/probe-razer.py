#!/usr/bin/env python3
"""Probe all Razer DeathAdder V2 hidraw interfaces to find which accepts feature reports."""

import os
import fcntl
import time
import glob

RAZER_VID = "1532"
RAZER_PID = "0084"

# ioctl numbers for HID feature reports (x86-64 Linux)
# _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, 91) and _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, 91)
HIDIOCSFEATURE = 0xC05B4806
HIDIOCGFEATURE = 0xC05B4807


def find_razer_hidraw():
    """Find all hidraw nodes matching the DeathAdder V2."""
    nodes = []
    for sysfs in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
        uevent_path = os.path.join(sysfs, "device/uevent")
        if not os.path.exists(uevent_path):
            continue
        uevent = open(uevent_path).read()
        if RAZER_VID not in uevent or RAZER_PID not in uevent:
            continue

        name = os.path.basename(sysfs)
        # Find interface number
        try:
            device_link = os.path.realpath(os.path.join(sysfs, "device"))
            iface = "?"
            cur = device_link
            for _ in range(10):
                candidate = os.path.join(cur, "bInterfaceNumber")
                if os.path.exists(candidate):
                    iface = open(candidate).read().strip()
                    break
                cur = os.path.dirname(cur)
        except Exception:
            iface = "?"

        nodes.append((name, iface))
    return nodes


def build_razer_packet(cmd_class, cmd_id, data_size, args):
    pkt = bytearray(90)
    pkt[1] = 0x1F  # transaction ID
    pkt[5] = data_size
    pkt[6] = cmd_class
    pkt[7] = cmd_id
    for i, b in enumerate(args[:80]):
        pkt[8 + i] = b
    crc = 0
    for i in range(2, 88):
        crc ^= pkt[i]
    pkt[88] = crc
    return pkt


def probe_interface(devpath):
    """Try sending a get_polling_rate command and see what happens."""
    try:
        fd = os.open(devpath, os.O_RDWR)
    except OSError as e:
        return f"open failed: {e}"

    try:
        pkt = build_razer_packet(0x00, 0x85, 0x01, [0x00])
        buf = bytearray(b'\x00') + pkt  # report ID prefix
        buf = bytearray(buf)

        try:
            fcntl.ioctl(fd, HIDIOCSFEATURE, buf)
        except OSError as e:
            return f"SET_REPORT failed: {e}"

        time.sleep(0.001)

        rbuf = bytearray(91)
        rbuf[0] = 0x00
        try:
            fcntl.ioctl(fd, HIDIOCGFEATURE, rbuf)
        except OSError as e:
            return f"GET_REPORT failed: {e}"

        resp = rbuf[1:]
        status = resp[0]
        poll_byte = resp[8]
        rates = {0x01: 1000, 0x02: 500, 0x04: 250, 0x08: 125}
        rate = rates.get(poll_byte, f"unknown ({poll_byte:#04x})")

        return (
            f"OK -- status={status:#04x} polling_rate={rate}Hz  "
            f"raw[0:12]={' '.join(f'{b:02x}' for b in resp[:12])}"
        )
    finally:
        os.close(fd)


if __name__ == "__main__":
    nodes = find_razer_hidraw()
    if not nodes:
        print("No Razer DeathAdder V2 hidraw nodes found")
        raise SystemExit(1)

    print(f"Found {len(nodes)} hidraw nodes for DeathAdder V2:\n")
    for name, iface in nodes:
        devpath = f"/dev/{name}"
        result = probe_interface(devpath)
        print(f"  {name} (interface {iface}): {result}")
