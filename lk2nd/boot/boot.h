/* SPDX-License-Identifier: BSD-3-Clause */
#ifndef LK2ND_BOOT_BOOT_H
#define LK2ND_BOOT_BOOT_H

#include <list.h>
#include <string.h>

#include <lk2nd/boot.h>

/* util.c */
void lk2nd_print_file_tree(char *root, char *prefix);

/* extlinux.c */
struct extlinux_label {
    const char *kernel;
    const char *initramfs;
    const char *dtb;
    const char *dtbdir;
    const char **dtboverlays;
    const char *cmdline;
};

void lk2nd_try_extlinux(const char *mountpoint);
int extlinux_parse_conf(char *data, size_t size, struct extlinux_label *label);
bool extlinux_expand_conf(struct extlinux_label *label, const char *root);
void extlinux_boot_label(struct extlinux_label *label);

#endif /* LK2ND_BOOT_BOOT_H */
