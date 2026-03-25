MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 24K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

__bootloader_state_start = 0x10006100;
__bootloader_state_end   = 0x10007000;

__bootloader_active_start = 0x10007000;
__bootloader_active_end   = 0x10087000;

__bootloader_dfu_start = 0x10087000;
__bootloader_dfu_end   = 0x10108000;
