//! Conversion from product configuration to `VZVirtualMachineConfiguration`.

use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2_foundation::{NSArray, NSData, NSString};
use objc2_virtualization::{
    VZAudioDeviceConfiguration, VZBridgedNetworkDeviceAttachment, VZBridgedNetworkInterface,
    VZDiskImageCachingMode, VZDiskImageStorageDeviceAttachment, VZDiskImageSynchronizationMode,
    VZEFIBootLoader, VZEFIVariableStore, VZEFIVariableStoreInitializationOptions,
    VZFileSerialPortAttachment, VZGenericMachineIdentifier, VZGenericPlatformConfiguration,
    VZGraphicsDeviceConfiguration, VZHostAudioInputStreamSource, VZHostAudioOutputStreamSink,
    VZKeyboardConfiguration, VZLinuxBootLoader, VZMACAddress, VZMacAuxiliaryStorage,
    VZMacGraphicsDeviceConfiguration, VZMacGraphicsDisplayConfiguration, VZMacHardwareModel,
    VZMacKeyboardConfiguration, VZMacMachineIdentifier, VZMacOSBootLoader,
    VZMacPlatformConfiguration, VZMacTrackpadConfiguration, VZMemoryBalloonDeviceConfiguration,
    VZNATNetworkDeviceAttachment, VZNetworkDeviceConfiguration, VZPlatformConfiguration,
    VZPointingDeviceConfiguration, VZSerialPortConfiguration, VZSharedDirectory,
    VZSingleDirectoryShare, VZSocketDeviceConfiguration, VZStorageDeviceConfiguration,
    VZUSBControllerConfiguration, VZUSBKeyboardConfiguration,
    VZUSBScreenCoordinatePointingDeviceConfiguration, VZVirtioBlockDeviceConfiguration,
    VZVirtioConsoleDeviceSerialPortConfiguration, VZVirtioEntropyDeviceConfiguration,
    VZVirtioFileSystemDeviceConfiguration, VZVirtioGraphicsDeviceConfiguration,
    VZVirtioGraphicsScanoutConfiguration, VZVirtioNetworkDeviceConfiguration,
    VZVirtioSocketDeviceConfiguration, VZVirtioSoundDeviceConfiguration,
    VZVirtioSoundDeviceInputStreamConfiguration, VZVirtioSoundDeviceOutputStreamConfiguration,
    VZVirtioSoundDeviceStreamConfiguration, VZVirtioTraditionalMemoryBalloonDeviceConfiguration,
    VZVirtualMachineConfiguration, VZXHCIControllerConfiguration,
};

use crate::error::{exception_error, framework_error, VzError};
use crate::util::{nsstring, path_to_nsurl, read_blob, write_blob};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VzBoot {
    Linux {
        kernel: PathBuf,
        initrd: Option<PathBuf>,
        command_line: String,
    },
    Efi {
        variable_store: PathBuf,
    },
    MacOs {
        hardware_model: PathBuf,
        machine_identifier: PathBuf,
        auxiliary_storage: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzDisk {
    pub path: PathBuf,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VzNetworkMode {
    Nat,
    Bridged { interface: Option<String> },
    Vmnet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzNetwork {
    pub mac: Option<String>,
    pub mode: VzNetworkMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzShare {
    pub tag: String,
    pub host_path: PathBuf,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzSerial {
    pub path: PathBuf,
    pub append: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VzGraphics {
    pub width: usize,
    pub height: usize,
    pub pixels_per_inch: usize,
}

impl Default for VzGraphics {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1200,
            pixels_per_inch: 80,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VzPointingKind {
    Usb,
    Trackpad,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VzKeyboardKind {
    Usb,
    Mac,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzConsolePort {
    pub name: String,
    pub log: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VzSpec {
    pub cpus: u32,
    pub memory_mib: u64,
    pub boot: VzBoot,
    pub disks: Vec<VzDisk>,
    pub networks: Vec<VzNetwork>,
    pub shares: Vec<VzShare>,
    pub serial: Option<VzSerial>,
    pub platform_identifier: Option<PathBuf>,
    pub graphics: Vec<VzGraphics>,
    pub audio: bool,
    pub usb: bool,
    pub pointing: VzPointingKind,
    pub keyboard: VzKeyboardKind,
    pub console_ports: Vec<VzConsolePort>,
    pub balloon: bool,
    pub socket: bool,
    pub entropy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgedInterface {
    pub identifier: String,
    pub display_name: Option<String>,
}

/// List host interfaces that Virtualization.framework can bridge to.
pub fn bridged_interfaces() -> Vec<BridgedInterface> {
    let interfaces = unsafe { VZBridgedNetworkInterface::networkInterfaces() };
    interfaces
        .iter()
        .map(|interface| BridgedInterface {
            // SAFETY: both properties are read-only framework queries on a
            // valid interface object.
            identifier: unsafe { interface.identifier() }.to_string(),
            display_name: unsafe { interface.localizedDisplayName() }.map(|name| name.to_string()),
        })
        .collect()
}

/// Build and validate a `VZVirtualMachineConfiguration`.
pub(crate) fn build_configuration(
    spec: &VzSpec,
) -> Result<Retained<VZVirtualMachineConfiguration>, VzError> {
    let result = objc2::exception::catch(|| build_inner(spec));
    match result {
        Ok(value) => value,
        Err(exception) => Err(exception_error(
            "configuring virtual machine",
            exception.as_deref(),
        )),
    }
}

fn build_inner(spec: &VzSpec) -> Result<Retained<VZVirtualMachineConfiguration>, VzError> {
    if spec.cpus == 0 {
        return Err(VzError::invalid_config("CPU count must be at least 1"));
    }
    if spec.memory_mib == 0 {
        return Err(VzError::invalid_config("memory must be greater than zero"));
    }
    let memory_bytes = spec
        .memory_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| VzError::invalid_config("memory size overflows"))?;

    // SAFETY: allocating a configuration object is always valid.
    let config = unsafe { VZVirtualMachineConfiguration::new() };

    // SAFETY: limits are static framework queries.
    let (min_cpus, max_cpus) = unsafe {
        (
            VZVirtualMachineConfiguration::minimumAllowedCPUCount(),
            VZVirtualMachineConfiguration::maximumAllowedCPUCount(),
        )
    };
    // SAFETY: limits are static framework queries.
    let (min_mem, max_mem) = unsafe {
        (
            VZVirtualMachineConfiguration::minimumAllowedMemorySize(),
            VZVirtualMachineConfiguration::maximumAllowedMemorySize(),
        )
    };
    if (spec.cpus as usize) < min_cpus || (spec.cpus as usize) > max_cpus {
        return Err(VzError::invalid_config(format!(
            "CPU count {} is outside the supported range {min_cpus}..={max_cpus}",
            spec.cpus
        )));
    }
    if memory_bytes < min_mem || memory_bytes > max_mem {
        return Err(VzError::invalid_config(format!(
            "memory {} MiB is outside the supported range {}..={} MiB",
            spec.memory_mib,
            min_mem / (1024 * 1024),
            max_mem / (1024 * 1024)
        )));
    }

    // SAFETY: values have been range-checked above.
    unsafe {
        config.setCPUCount(spec.cpus as usize);
        config.setMemorySize(memory_bytes);
    }

    install_platform(spec, &config)?;
    install_boot_loader(spec, &config)?;
    install_disks(spec, &config)?;
    install_networks(spec, &config)?;
    install_shares(spec, &config)?;
    install_serial(spec, &config)?;

    // A virtio entropy device gives the guest a reliable /dev/random.
    if spec.entropy {
        // SAFETY: default entropy device construction is valid.
        let entropy = unsafe { VZVirtioEntropyDeviceConfiguration::new() };
        let entropy_ref: &objc2_virtualization::VZEntropyDeviceConfiguration = &entropy;
        let entropy_array = NSArray::from_slice(&[entropy_ref]);
        // SAFETY: the array contains one valid entropy configuration.
        unsafe { config.setEntropyDevices(&entropy_array) };
    }

    install_graphics(spec, &config)?;
    install_audio(spec, &config)?;
    install_usb(spec, &config)?;
    install_pointing(spec, &config)?;
    install_keyboard(spec, &config)?;
    install_balloon(spec, &config)?;
    install_socket(spec, &config)?;

    // SAFETY: validateWithError reports configuration problems as an NSError.
    unsafe { config.validateWithError() }
        .map_err(|error| framework_error("validating VM configuration", &error))?;

    Ok(config)
}

fn install_platform(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    match &spec.boot {
        VzBoot::MacOs {
            hardware_model,
            machine_identifier,
            auxiliary_storage,
        } => {
            for (label, path) in [
                ("hardware model", hardware_model),
                ("machine identifier", machine_identifier),
                ("auxiliary storage", auxiliary_storage),
            ] {
                if !path.is_file() {
                    return Err(VzError::invalid_config(format!(
                        "macOS {label} is missing: {}",
                        path.display()
                    )));
                }
            }
            let hardware = load_hardware_model(hardware_model)?;
            // SAFETY: hardware model support is a read-only query.
            if !unsafe { hardware.isSupported() } {
                return Err(VzError::invalid_config(
                    "this Mac does not support the configured macOS hardware model",
                ));
            }
            let identifier = load_mac_identifier(machine_identifier)?;
            let aux_url = path_to_nsurl(auxiliary_storage)?;
            // SAFETY: the URL points to an existing auxiliary storage image.
            let aux = unsafe {
                VZMacAuxiliaryStorage::initWithURL(VZMacAuxiliaryStorage::alloc(), &aux_url)
            };

            // SAFETY: default platform construction is valid.
            let platform = unsafe { VZMacPlatformConfiguration::new() };
            // SAFETY: all three values are valid and retained by the platform.
            unsafe {
                platform.setHardwareModel(&hardware);
                platform.setMachineIdentifier(&identifier);
                platform.setAuxiliaryStorage(Some(&aux));
            }
            let platform_ref: &VZPlatformConfiguration = &platform;
            // SAFETY: platform is a VZMacPlatformConfiguration subclass.
            unsafe { config.setPlatform(platform_ref) };

            if !spec.graphics.is_empty() {
                install_mac_graphics(spec, config)?;
            }
        }
        _ => {
            // SAFETY: default generic platform construction is valid.
            let platform = unsafe { VZGenericPlatformConfiguration::new() };
            if let Some(path) = &spec.platform_identifier {
                let identifier = load_or_create_generic_identifier(path, &platform)?;
                // SAFETY: identifier was created by the same framework.
                unsafe { platform.setMachineIdentifier(&identifier) };
            }
            let platform_ref: &VZPlatformConfiguration = &platform;
            // SAFETY: platform is a VZGenericPlatformConfiguration subclass.
            unsafe { config.setPlatform(platform_ref) };
        }
    }
    Ok(())
}

fn install_boot_loader(
    spec: &VzSpec,
    config: &VZVirtualMachineConfiguration,
) -> Result<(), VzError> {
    match &spec.boot {
        VzBoot::Linux {
            kernel,
            initrd,
            command_line,
        } => {
            if !kernel.is_file() {
                return Err(VzError::invalid_config(format!(
                    "Linux kernel not found: {}",
                    kernel.display()
                )));
            }
            if let Some(initrd) = initrd {
                if !initrd.is_file() {
                    return Err(VzError::invalid_config(format!(
                        "initial ramdisk not found: {}",
                        initrd.display()
                    )));
                }
            }
            let kernel_url = path_to_nsurl(kernel)?;
            // SAFETY: initWithKernelURL is valid for an existing file URL.
            let loader = unsafe {
                VZLinuxBootLoader::initWithKernelURL(VZLinuxBootLoader::alloc(), &kernel_url)
            };
            if let Some(initrd) = initrd {
                let initrd_url = path_to_nsurl(initrd)?;
                // SAFETY: valid file URL.
                unsafe { loader.setInitialRamdiskURL(Some(&initrd_url)) };
            }
            let command_line = nsstring(command_line);
            // SAFETY: valid string.
            unsafe { loader.setCommandLine(&command_line) };
            let loader_ref: &objc2_virtualization::VZBootLoader = &loader;
            // SAFETY: loader is a VZLinuxBootLoader subclass.
            unsafe { config.setBootLoader(Some(loader_ref)) };
        }
        VzBoot::Efi { variable_store } => {
            ensure_parent(variable_store)?;
            let url = path_to_nsurl(variable_store)?;
            let store = if variable_store.is_file() {
                // SAFETY: existing variable store file.
                unsafe { VZEFIVariableStore::initWithURL(VZEFIVariableStore::alloc(), &url) }
            } else {
                // SAFETY: creating a new store at a path whose parent exists.
                unsafe {
                    VZEFIVariableStore::initCreatingVariableStoreAtURL_options_error(
                        VZEFIVariableStore::alloc(),
                        &url,
                        VZEFIVariableStoreInitializationOptions(0),
                    )
                }
                .map_err(|error| framework_error("creating EFI variable store", &error))?
            };
            // SAFETY: default EFI boot loader construction is valid.
            let loader = unsafe { VZEFIBootLoader::new() };
            // SAFETY: store is a valid VZEFIVariableStore.
            unsafe { loader.setVariableStore(Some(&store)) };
            let loader_ref: &objc2_virtualization::VZBootLoader = &loader;
            // SAFETY: loader is a VZEFIBootLoader subclass.
            unsafe { config.setBootLoader(Some(loader_ref)) };
        }
        VzBoot::MacOs { .. } => {
            // SAFETY: default macOS boot loader construction is valid.
            let loader = unsafe { VZMacOSBootLoader::new() };
            let loader_ref: &objc2_virtualization::VZBootLoader = &loader;
            // SAFETY: loader is a VZMacOSBootLoader subclass.
            unsafe { config.setBootLoader(Some(loader_ref)) };
        }
    }
    Ok(())
}

fn install_disks(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    let mut devices = Vec::with_capacity(spec.disks.len());
    for disk in &spec.disks {
        if !disk.path.is_file() {
            return Err(VzError::invalid_config(format!(
                "disk image not found: {}",
                disk.path.display()
            )));
        }
        let url = path_to_nsurl(&disk.path)?;
        // SAFETY: the URL points to an existing raw image. Cached + Full
        // synchronization avoids the ext4 corruption seen with the automatic
        // mode under heavy guest writeback.
        let attachment = unsafe {
            VZDiskImageStorageDeviceAttachment::initWithURL_readOnly_cachingMode_synchronizationMode_error(
                VZDiskImageStorageDeviceAttachment::alloc(),
                &url,
                disk.read_only,
                VZDiskImageCachingMode::Cached,
                VZDiskImageSynchronizationMode::Full,
            )
        }
        .map_err(|error| framework_error("attaching disk image", &error))?;
        // SAFETY: attachment is valid.
        let device = unsafe {
            VZVirtioBlockDeviceConfiguration::initWithAttachment(
                VZVirtioBlockDeviceConfiguration::alloc(),
                &attachment,
            )
        };
        devices.push(device);
    }
    if !devices.is_empty() {
        let refs: Vec<&VZStorageDeviceConfiguration> = devices
            .iter()
            .map(|device| device as &VZStorageDeviceConfiguration)
            .collect();
        let array = NSArray::from_slice(&refs);
        // SAFETY: all entries are valid storage device configurations.
        unsafe { config.setStorageDevices(&array) };
    }
    Ok(())
}

fn install_networks(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    let mut devices = Vec::with_capacity(spec.networks.len());
    for network in &spec.networks {
        // SAFETY: default virtio network device construction is valid.
        let device = unsafe { VZVirtioNetworkDeviceConfiguration::new() };

        match &network.mode {
            VzNetworkMode::Nat => {
                // SAFETY: default NAT attachment construction is valid.
                let attachment = unsafe { VZNATNetworkDeviceAttachment::new() };
                // SAFETY: valid attachment.
                unsafe { device.setAttachment(Some(&attachment)) };
            }
            VzNetworkMode::Bridged { interface } => {
                let interface = resolve_bridged_interface(interface.as_deref())?;
                // SAFETY: interface came from the framework's own list.
                let attachment = unsafe {
                    VZBridgedNetworkDeviceAttachment::initWithInterface(
                        VZBridgedNetworkDeviceAttachment::alloc(),
                        &interface,
                    )
                };
                // SAFETY: valid attachment.
                unsafe { device.setAttachment(Some(&attachment)) };
            }
            VzNetworkMode::Vmnet => {
                return Err(VzError::Unsupported);
            }
        }

        if let Some(mac) = &network.mac {
            let mac_string = nsstring(mac);
            // SAFETY: initWithString returns nil for malformed addresses.
            let mac = unsafe { VZMACAddress::initWithString(VZMACAddress::alloc(), &mac_string) }
                .ok_or_else(|| {
                VzError::invalid_config(format!("invalid MAC address {mac:?}"))
            })?;
            // SAFETY: valid MAC address.
            unsafe { device.setMACAddress(&mac) };
        }
        devices.push(device);
    }
    if !devices.is_empty() {
        let refs: Vec<&VZNetworkDeviceConfiguration> = devices
            .iter()
            .map(|device| device as &VZNetworkDeviceConfiguration)
            .collect();
        let array = NSArray::from_slice(&refs);
        // SAFETY: all entries are valid network device configurations.
        unsafe { config.setNetworkDevices(&array) };
    }
    Ok(())
}

fn install_shares(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    let mut devices = Vec::with_capacity(spec.shares.len());
    for share in &spec.shares {
        if !share.host_path.is_dir() {
            return Err(VzError::invalid_config(format!(
                "shared directory does not exist: {}",
                share.host_path.display()
            )));
        }
        let url = path_to_nsurl(&share.host_path)?;
        // SAFETY: existing directory URL.
        let directory = unsafe {
            VZSharedDirectory::initWithURL_readOnly(
                VZSharedDirectory::alloc(),
                &url,
                share.read_only,
            )
        };
        // SAFETY: valid directory share.
        let single = unsafe {
            VZSingleDirectoryShare::initWithDirectory(VZSingleDirectoryShare::alloc(), &directory)
        };
        let tag = nsstring(&share.tag);
        // SAFETY: tag validity is checked by the framework in the initializer.
        let device = unsafe {
            VZVirtioFileSystemDeviceConfiguration::initWithTag(
                VZVirtioFileSystemDeviceConfiguration::alloc(),
                &tag,
            )
        };
        // SAFETY: valid directory share.
        unsafe { device.setShare(Some(&single)) };
        devices.push(device);
    }
    if !devices.is_empty() {
        let refs: Vec<&objc2_virtualization::VZDirectorySharingDeviceConfiguration> = devices
            .iter()
            .map(|device| device as &objc2_virtualization::VZDirectorySharingDeviceConfiguration)
            .collect();
        let array = NSArray::from_slice(&refs);
        // SAFETY: all entries are valid directory sharing configurations.
        unsafe { config.setDirectorySharingDevices(&array) };
    }
    Ok(())
}

fn install_serial(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    let Some(serial) = &spec.serial else {
        return Ok(());
    };
    ensure_parent(&serial.path)?;
    let url = path_to_nsurl(&serial.path)?;
    // SAFETY: the parent directory exists and the file may be created by VZ.
    let attachment = unsafe {
        VZFileSerialPortAttachment::initWithURL_append_error(
            VZFileSerialPortAttachment::alloc(),
            &url,
            serial.append,
        )
    }
    .map_err(|error| framework_error("opening serial log", &error))?;
    // SAFETY: default virtio console serial port construction is valid.
    let port = unsafe { VZVirtioConsoleDeviceSerialPortConfiguration::new() };
    // SAFETY: valid attachment.
    unsafe { port.setAttachment(Some(&attachment)) };
    let port_ref: &VZSerialPortConfiguration = &port;
    let array = NSArray::from_slice(&[port_ref]);
    // SAFETY: array contains one valid serial port.
    unsafe { config.setSerialPorts(&array) };
    Ok(())
}

fn install_mac_graphics(
    spec: &VzSpec,
    config: &VZVirtualMachineConfiguration,
) -> Result<(), VzError> {
    if spec.graphics.is_empty() {
        return Ok(());
    }
    let mut displays = Vec::with_capacity(spec.graphics.len());
    for graphics in &spec.graphics {
        // SAFETY: display construction only uses plain integers.
        let display = unsafe {
            VZMacGraphicsDisplayConfiguration::initWithWidthInPixels_heightInPixels_pixelsPerInch(
                VZMacGraphicsDisplayConfiguration::alloc(),
                graphics.width as isize,
                graphics.height as isize,
                graphics.pixels_per_inch as isize,
            )
        };
        displays.push(display);
    }
    let display_refs: Vec<&VZMacGraphicsDisplayConfiguration> =
        displays.iter().map(|d| &**d).collect();
    let displays_array = NSArray::from_slice(&display_refs);
    // SAFETY: default macOS graphics device construction is valid.
    let device = unsafe { VZMacGraphicsDeviceConfiguration::new() };
    // SAFETY: array contains valid displays.
    unsafe { device.setDisplays(&displays_array) };
    let device_ref: &VZGraphicsDeviceConfiguration = &device;
    let array = NSArray::from_slice(&[device_ref]);
    // SAFETY: array contains one valid graphics device.
    unsafe { config.setGraphicsDevices(&array) };
    Ok(())
}

fn install_graphics(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    if spec.graphics.is_empty() {
        return Ok(());
    }
    if matches!(spec.boot, VzBoot::MacOs { .. }) {
        return Ok(());
    }
    let mut scanouts = Vec::with_capacity(spec.graphics.len());
    for graphics in &spec.graphics {
        // SAFETY: scanout construction only uses plain integers.
        let scanout = unsafe {
            VZVirtioGraphicsScanoutConfiguration::initWithWidthInPixels_heightInPixels(
                VZVirtioGraphicsScanoutConfiguration::alloc(),
                graphics.width as isize,
                graphics.height as isize,
            )
        };
        scanouts.push(scanout);
    }
    let scanout_refs: Vec<&VZVirtioGraphicsScanoutConfiguration> =
        scanouts.iter().map(|s| &**s).collect();
    let scanouts_array = NSArray::from_slice(&scanout_refs);
    // SAFETY: default virtio graphics device construction is valid.
    let device = unsafe { VZVirtioGraphicsDeviceConfiguration::new() };
    // SAFETY: array contains valid scanouts.
    unsafe { device.setScanouts(&scanouts_array) };
    let device_ref: &VZGraphicsDeviceConfiguration = &device;
    let array = NSArray::from_slice(&[device_ref]);
    // SAFETY: array contains one valid graphics device.
    unsafe { config.setGraphicsDevices(&array) };
    Ok(())
}

fn install_audio(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    if !spec.audio {
        return Ok(());
    }
    let mut retained_inputs: Vec<Retained<VZVirtioSoundDeviceInputStreamConfiguration>> =
        Vec::new();
    let mut retained_outputs: Vec<Retained<VZVirtioSoundDeviceOutputStreamConfiguration>> =
        Vec::new();
    let mut refs: Vec<&VZVirtioSoundDeviceStreamConfiguration> = Vec::new();

    // SAFETY: default host audio source/sink construction is valid.
    let input_stream = unsafe { VZVirtioSoundDeviceInputStreamConfiguration::new() };
    let source = unsafe { VZHostAudioInputStreamSource::new() };
    unsafe { input_stream.setSource(Some(&source)) };
    retained_inputs.push(input_stream);
    refs.push(retained_inputs.last().unwrap() as &VZVirtioSoundDeviceStreamConfiguration);

    // SAFETY: default host audio sink construction is valid.
    let output_stream = unsafe { VZVirtioSoundDeviceOutputStreamConfiguration::new() };
    let sink = unsafe { VZHostAudioOutputStreamSink::new() };
    unsafe { output_stream.setSink(Some(&sink)) };
    retained_outputs.push(output_stream);
    refs.push(retained_outputs.last().unwrap() as &VZVirtioSoundDeviceStreamConfiguration);

    let stream_array = NSArray::from_slice(&refs);
    // SAFETY: default virtio sound device construction is valid.
    let device = unsafe { VZVirtioSoundDeviceConfiguration::new() };
    // SAFETY: array contains valid stream configurations.
    unsafe { device.setStreams(&stream_array) };
    let device_ref: &VZAudioDeviceConfiguration = &device;
    let array = NSArray::from_slice(&[device_ref]);
    // SAFETY: array contains one valid audio device.
    unsafe { config.setAudioDevices(&array) };

    let _ = (retained_inputs, retained_outputs);
    Ok(())
}

fn install_usb(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    if !spec.usb {
        return Ok(());
    }
    // SAFETY: default XHCI controller construction is valid.
    let controller = unsafe { VZXHCIControllerConfiguration::new() };
    let controller_ref: &VZUSBControllerConfiguration = &controller;
    let array = NSArray::from_slice(&[controller_ref]);
    // SAFETY: array contains one valid USB controller.
    unsafe { config.setUsbControllers(&array) };
    Ok(())
}

fn install_pointing(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    match spec.pointing {
        VzPointingKind::None => Ok(()),
        VzPointingKind::Usb => {
            // SAFETY: default USB pointing device construction is valid.
            let device = unsafe { VZUSBScreenCoordinatePointingDeviceConfiguration::new() };
            let device_ref: &VZPointingDeviceConfiguration = &device;
            let array = NSArray::from_slice(&[device_ref]);
            // SAFETY: array contains one valid pointing device.
            unsafe { config.setPointingDevices(&array) };
            Ok(())
        }
        VzPointingKind::Trackpad => {
            // SAFETY: default trackpad construction is valid.
            let device = unsafe { VZMacTrackpadConfiguration::new() };
            let device_ref: &VZPointingDeviceConfiguration = &device;
            let array = NSArray::from_slice(&[device_ref]);
            // SAFETY: array contains one valid pointing device.
            unsafe { config.setPointingDevices(&array) };
            Ok(())
        }
    }
}

fn install_keyboard(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    match spec.keyboard {
        VzKeyboardKind::None => Ok(()),
        VzKeyboardKind::Usb => {
            // SAFETY: default USB keyboard construction is valid.
            let device = unsafe { VZUSBKeyboardConfiguration::new() };
            let device_ref: &VZKeyboardConfiguration = &device;
            let array = NSArray::from_slice(&[device_ref]);
            // SAFETY: array contains one valid keyboard.
            unsafe { config.setKeyboards(&array) };
            Ok(())
        }
        VzKeyboardKind::Mac => {
            // SAFETY: default Mac keyboard construction is valid.
            let device = unsafe { VZMacKeyboardConfiguration::new() };
            let device_ref: &VZKeyboardConfiguration = &device;
            let array = NSArray::from_slice(&[device_ref]);
            // SAFETY: array contains one valid keyboard.
            unsafe { config.setKeyboards(&array) };
            Ok(())
        }
    }
}

fn install_balloon(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    if !spec.balloon {
        return Ok(());
    }
    // SAFETY: default balloon device construction is valid.
    let device = unsafe { VZVirtioTraditionalMemoryBalloonDeviceConfiguration::new() };
    let device_ref: &VZMemoryBalloonDeviceConfiguration = &device;
    let array = NSArray::from_slice(&[device_ref]);
    // SAFETY: array contains one valid balloon device.
    unsafe { config.setMemoryBalloonDevices(&array) };
    Ok(())
}

fn install_socket(spec: &VzSpec, config: &VZVirtualMachineConfiguration) -> Result<(), VzError> {
    if !spec.socket {
        return Ok(());
    }
    // SAFETY: default vsock device construction is valid.
    let device = unsafe { VZVirtioSocketDeviceConfiguration::new() };
    let device_ref: &VZSocketDeviceConfiguration = &device;
    let array = NSArray::from_slice(&[device_ref]);
    // SAFETY: array contains one valid socket device.
    unsafe { config.setSocketDevices(&array) };
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<(), VzError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| VzError::io(parent, e))?;
        }
    }
    Ok(())
}

fn load_hardware_model(path: &Path) -> Result<Retained<VZMacHardwareModel>, VzError> {
    let bytes = read_blob(path)?;
    if bytes.is_empty() {
        return Err(VzError::invalid_config(format!(
            "hardware model is empty: {}",
            path.display()
        )));
    }
    let data = NSData::with_bytes(&bytes);
    // SAFETY: data is a copy of previously serialized framework data.
    unsafe { VZMacHardwareModel::initWithDataRepresentation(VZMacHardwareModel::alloc(), &data) }
        .ok_or_else(|| {
            VzError::invalid_config(format!("invalid hardware model: {}", path.display()))
        })
}

fn load_mac_identifier(path: &Path) -> Result<Retained<VZMacMachineIdentifier>, VzError> {
    let bytes = read_blob(path)?;
    if bytes.is_empty() {
        return Err(VzError::invalid_config(format!(
            "machine identifier is empty: {}",
            path.display()
        )));
    }
    let data = NSData::with_bytes(&bytes);
    // SAFETY: data is a copy of previously serialized framework data.
    unsafe {
        VZMacMachineIdentifier::initWithDataRepresentation(VZMacMachineIdentifier::alloc(), &data)
    }
    .ok_or_else(|| {
        VzError::invalid_config(format!(
            "invalid macOS machine identifier: {}",
            path.display()
        ))
    })
}

fn load_or_create_generic_identifier(
    path: &Path,
    platform: &VZGenericPlatformConfiguration,
) -> Result<Retained<VZGenericMachineIdentifier>, VzError> {
    if path.is_file() {
        let bytes = read_blob(path)?;
        if bytes.is_empty() {
            return Err(VzError::invalid_config(format!(
                "machine identifier is empty: {}",
                path.display()
            )));
        }
        let data = NSData::with_bytes(&bytes);
        // SAFETY: data is a copy of previously serialized framework data.
        return unsafe {
            VZGenericMachineIdentifier::initWithDataRepresentation(
                VZGenericMachineIdentifier::alloc(),
                &data,
            )
        }
        .ok_or_else(|| {
            VzError::invalid_config(format!("invalid machine identifier: {}", path.display()))
        });
    }

    // SAFETY: machineIdentifier is populated by the framework.
    let identifier = unsafe { platform.machineIdentifier() };
    // SAFETY: dataRepresentation returns an immutable byte blob.
    let bytes = unsafe { identifier.dataRepresentation() }.to_vec();
    write_blob(path, &bytes)?;
    Ok(identifier)
}

fn resolve_bridged_interface(
    requested: Option<&str>,
) -> Result<Retained<VZBridgedNetworkInterface>, VzError> {
    // SAFETY: static interface list query.
    let interfaces = unsafe { VZBridgedNetworkInterface::networkInterfaces() };
    let available: Vec<String> = interfaces
        .iter()
        .map(|interface| {
            // SAFETY: read-only query on a valid interface object.
            unsafe { interface.identifier() }.to_string()
        })
        .collect();

    match requested {
        Some(name) => interfaces
            .iter()
            .find(|interface| {
                // SAFETY: read-only query on a valid interface object.
                unsafe { interface.identifier() }.to_string() == name
            })
            .ok_or_else(|| {
                VzError::invalid_config(format!(
                    "bridged interface {name:?} not found (available: {})",
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                ))
            }),
        None => {
            if interfaces.len() == 1 {
                interfaces.iter().next().ok_or_else(|| {
                    VzError::invalid_config("no bridged host interfaces are available")
                })
            } else if interfaces.is_empty() {
                Err(VzError::invalid_config(
                    "no bridged host interfaces are available",
                ))
            } else {
                Err(VzError::invalid_config(format!(
                    "multiple bridged interfaces are available; choose one with --bridge (available: {})",
                    available.join(", ")
                )))
            }
        }
    }
}

/// Validate a virtiofs tag without constructing a full VM.
pub fn validate_share_tag(tag: &str) -> Result<(), VzError> {
    let tag = NSString::from_str(tag);
    // SAFETY: static validation helper.
    unsafe { VZVirtioFileSystemDeviceConfiguration::validateTag_error(&tag) }
        .map_err(|error| framework_error("validating share tag", &error))
}
