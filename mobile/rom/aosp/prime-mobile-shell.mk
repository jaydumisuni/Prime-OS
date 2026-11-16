# Include from a qualified Prime Mobile AOSP product after device/vendor admission.
# Not a stand-alone BoardConfig, not an OTA/flash/install script.
# This dependency is purely the graphical application surface; it does NOT
# install Linux primed, PHI background AI services or experimental boot images.
PRODUCT_PACKAGES += PrimeMobileShell
