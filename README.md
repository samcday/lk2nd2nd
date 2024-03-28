# lk2nd2nd

**!UNDER CONSTRUCTION!** This README is aspirational, don't use this repo for anything yet unless you're a masochist neckbeard hacker person!

A fork of [lk2nd][1] that provides extended bootloader features.

**NOTE:** this bootloader currently *only* supports msm8916 devices. It is also **experimental** at this time. Please **do not** flash it to a device that doesn't provide a way to reflash the boot partition before this bootloader starts.

The maintainer has only tested it on the following devices:

 * [Samsung Galaxy A5 (2015)][2]
 * [Samsung Galaxy Tab A 9.7 (2015)][3]

## Features

This is a light fork of lk2nd, so it can do everything you can expect from lk2nd, plus the following features:

 * ESP partition support
 * Boot from UKI
 * (soon) Boot selection screen

## Why fork?

The upstream maintainer has made it clear that lk2nd is in "maintenance mode" and will not accept new features. The intent (as I understand it) is to focus on porting U-Boot to the msm8916 platforms, and then either use lk2nd to chainload U-Boot, or replace lk2nd with U-Boot entirely.

It's unclear when that will progress to a point of maturity such that it's recommended for the average user to boot via U-Boot, though. In the mean time, I really want to dual boot [Arch Linux](https://github.com/samcday/archlinux-msm8916/) and pmOS on my msm8916 devices.

Also, I wrote all the extensions in Rust, which probably makes the chances of upstreaming this even more slim.

That said, if anyone wants to make the effort to get this upstream, please be my guest! I wish you the best of luck.

[1]: https://github.com/msm8916-mainline/lk2nd
[2]: https://wiki.postmarketos.org/wiki/Samsung_Galaxy_A5_2015_(samsung-a5)
[3]: https://wiki.postmarketos.org/wiki/Samsung_Galaxy_Tab_A_9.7_2015_(samsung-gt510)
