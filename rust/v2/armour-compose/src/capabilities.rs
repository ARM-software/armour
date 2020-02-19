/*
  Support for Linux capabilities
  Uses some code from https://github.com/gcmurphy/rust-capabilities/blob/master/src/lib.rs

  Author: Anthony Fox
*/
use super::serde_utils::from_str;
use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet as Set;
use std::fmt;
use std::str::FromStr;

/// Capability descriptions taken from:
/// https://github.com/torvalds/linux/blob/master/include/uapi/linux/capability.h
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize)]
#[allow(non_camel_case_types)]
// #[serde(rename_all(serialize = "UPPERCASE"))]
pub enum Capability {
    /// In a system with the [_POSIX_CHOWN_RESTRICTED] option defined, this
    /// overrides the restriction of changing file ownership and group
    /// ownership.
    CHOWN = 0,

    /// Override all DAC access, including ACL execute access if
    /// [_POSIX_ACL] is defined. Excluding DAC access covered by
    /// LINUX_IMMUTABLE.
    DAC_OVERRIDE = 1,

    /// Overrides all DAC restrictions regarding read and search on files
    /// and directories, including ACL restrictions if [_POSIX_ACL] is
    /// defined. Excluding DAC access covered by LINUX_IMMUTABLE.
    DAC_READ_SEARCH = 2,

    /// Overrides all restrictions about allowed operations on files, where
    /// file owner ID must be equal to the user ID, except where FSETID
    /// is applicable. It doesn't override MAC and DAC restrictions.
    FOWNER = 3,

    /// Overrides the following restrictions that the effective user ID
    /// shall match the file owner ID when setting the S_ISUID and S_ISGID
    /// bits on that file; that the effective group ID (or one of the
    /// supplementary group IDs) shall match the file owner ID when setting
    /// the S_ISGID bit on that file; that the S_ISUID and S_ISGID bits are
    /// cleared on successful return from chown(2) (not implemented).
    FSETID = 4,

    /// Overrides the restriction that the real or effective user ID of a
    /// process sending a signal must match the real or effective user ID
    /// of the process receiving the signal.
    KILL = 5,

    /// - Allows setgid(2) manipulation
    /// - Allows setgroups(2) manipulation
    /// - Allows forged gids on socket credentials passing.
    SETGID = 6,

    /// - Allows set*uid(2) manipulation (including fsuid).
    /// - Allows forged pids on socket credentials passing.
    SETUID = 7,

    /// - Without VFS support for capabilities:
    ///   * Transfer any capability in your permitted set to any pid,
    ///     remove any capability in your permitted set from any pid.
    /// - With VFS support for capabilities (neither of above, but)
    ///   * Add any capability from current's capability bounding set
    ///     to the current process' inheritable set.
    ///   * Allow taking bits out of capability bounding set
    ///   * Allow modification of the securebits for a process
    SETPCAP = 8,

    /// Allow modification of S_IMMUTABLE and S_APPEND file attributes
    LINUX_IMMUTABLE = 9,

    /// - Allows binding to TCP/UDP sockets below 1024,
    /// - Allows binding to ATM VCIs below 32
    NET_BIND_SERVICE = 10,

    /// Allows broadcasting, listen to multicast
    NET_BROADCAST = 11,

    /// - Allow interface configuration
    /// - Allow administration of IP firewall, masquerading and accounting
    /// - Allow setting debug option on sockets
    /// - Allow modification of routing tables
    /// - Allow setting arbitrary process / process group ownership on sockets
    /// - Allow binding to any address for transparent proxying (also via NET_RAW)
    /// - Allow setting TOS (type of service)
    /// - Allow setting promiscuous mode
    /// - Allow clearing driver statistics
    /// - Allow multicasting
    /// - Allow read/write of device-specific registers
    /// - Allow activation of ATM control sockets
    NET_ADMIN = 12,

    /// - Allow use of RAW sockets
    /// - Allow use of PACKET sockets
    /// - Allow binding to any address for transparent proxying (also via NET_ADMIN)
    NET_RAW = 13,

    /// - Allow locking of shared memory segments
    /// - Allow mlock and mlockall (which doesn't really have anything to do with IPC)
    IPC_LOCK = 14,

    /// Override IPC ownership checks
    IPC_OWNER = 15,

    /// Insert and remove kernel modules - modify kernel without limit
    SYS_MODULE = 16,

    /// - Allow ioperm/iopl access
    /// - Allow sending USB messages to any device via /proc/bus/usb
    SYS_RAWIO = 17,

    /// Allow the use of chroot
    SYS_CHROOT = 18,

    /// Allow ptrace() of any process
    SYS_PTRACE = 19,

    /// Allow configuration of process accounting
    SYS_PACCT = 20,

    /// - Allow configuration of the secure attention key
    /// - Allow administration of the random device
    /// - Allow examination and configuration of disk quotas
    /// - Allow setting the domainname
    /// - Allow setting the hostname
    /// - Allow calling bdflush()
    /// - Allow mount() and umount(), setting up new smb connection
    /// - Allow some autofs root ioctls
    /// - Allow nfsservctl
    /// - Allow VM86_REQUEST_IRQ
    /// - Allow to read/write pci config on alpha
    /// - Allow irix_prctl on mips (setstacksize)
    /// - Allow flushing all cache on m68k (sys_cacheflush)
    /// - Allow removing semaphores
    /// - Used instead of CHOWN to "chown" IPC message queues, semaphores
    ///   and shared memory
    /// - Allow locking/unlocking of shared memory segment
    /// - Allow turning swap on/off
    /// - Allow forged pids on socket credentials passing
    /// - Allow setting readahead and flushing buffers on block devices
    /// - Allow setting geometry in floppy driver
    /// - Allow turning DMA on/off in xd driver
    /// - Allow administration of md devices (mostly the above, but some
    ///   extra ioctls)
    /// - Allow tuning the ide driver
    /// - Allow access to the nvram device
    /// - Allow administration of apm_bios, serial and bttv (TV) device
    /// - Allow manufacturer commands in isdn CAPI support driver
    /// - Allow reading non-standardized portions of pci configuration space
    /// - Allow DDI debug ioctl on sbpcd driver
    /// - Allow setting up serial ports
    /// - Allow sending raw qic-117 commands
    /// - Allow enabling/disabling tagged queuing on SCSI controllers and sending
    ///   arbitrary SCSI commands
    /// - Allow setting encryption key on loopback filesystem
    /// - Allow setting zone reclaim policy
    SYS_ADMIN = 21,

    /// Allow use of reboot()
    SYS_BOOT = 22,

    /// - Allow raising priority and setting priority on other (different
    ///   UID) processes
    /// - Allow use of FIFO and round-robin (realtime) scheduling on own
    ///   processes and setting the scheduling algorithm used by another
    ///   process.
    /// - Allow setting cpu affinity on other processes
    SYS_NICE = 23,

    /// - Override resource limits. Set resource limits.
    /// - Override quota limits.
    /// - Override reserved space on ext2 filesystem
    /// - Modify data journaling mode on ext3 filesystem (uses journaling
    ///   resources)
    /// - **NOTE**: *ext2 honors fsuid when checking for resource overrides, so
    ///   you can override using fsuid too.*
    /// - Override size restrictions on IPC message queues
    /// - Allow more than 64hz interrupts from the real-time clock
    /// - Override max number of consoles on console allocation
    /// - Override max number of keymaps
    SYS_RESOURCE = 24,

    /// - Allow manipulation of system clock
    /// - Allow irix_stime on mips
    /// - Allow setting the real-time clock
    SYS_TIME = 25,

    /// - Allow configuration of tty devices
    /// - Allow vhangup() of tty
    SYS_TTY_CONFIG = 26,

    /// Allow the privileged aspects of mknod()
    MKNOD = 27,

    /// Allow taking of leases on files
    LEASE = 28,

    /// Allow writing the audit log via unicast netlink socket
    AUDIT_WRITE = 29,

    /// Allow configurationof audit via unicast netlink socket
    AUDIT_CONTROL = 30,

    /// Set file capabilities
    SETFCAP = 31,

    /// Override MAC access.
    /// The base kernel enforces no MAC policy.
    /// An LSM may enforce a MAC policy, and if it does and it chooses
    /// to implement capability based overrides of that policy, this is
    /// the capability it should use to do so.
    MAC_OVERRIDE = 32,

    /// Allow MAC configuration or state changes.
    /// The base kernel requires no MAC configuration.
    /// An LSM may enforce a MAC policy, and if it does and it chooses
    /// to implement capability based checks on modifications to that
    /// policy or the data required to maintain it, this is the
    /// capability it should use to do so.
    MAC_ADMIN = 33,

    /// Allow configuring the kernel's syslog (printk behaviour)
    SYSLOG = 34,

    /// Allow triggering something that will wake the system
    WAKE_ALARM = 35,

    /// Allow preventing system suspends
    BLOCK_SUSPEND = 36,

    /// Allow reading the audit log via multicast netlink socket
    AUDIT_READ = 37,
}

impl FromStr for Capability {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Capability::*;
        match s.to_uppercase().as_str() {
            "AUDIT_CONTROL" => Ok(AUDIT_CONTROL),
            "AUDIT_READ" => Ok(AUDIT_READ),
            "AUDIT_WRITE" => Ok(AUDIT_WRITE),
            "BLOCK_SUSPEND" => Ok(BLOCK_SUSPEND),
            "CHOWN" => Ok(CHOWN),
            "DAC_OVERRIDE" => Ok(DAC_OVERRIDE),
            "DAC_READ_SEARCH" => Ok(DAC_READ_SEARCH),
            "FOWNER" => Ok(FOWNER),
            "FSETID" => Ok(FSETID),
            "IPC_LOCK" => Ok(IPC_LOCK),
            "IPC_OWNER" => Ok(IPC_OWNER),
            "KILL" => Ok(KILL),
            "LEASE" => Ok(LEASE),
            "LINUX_IMMUTABLE" => Ok(LINUX_IMMUTABLE),
            "MAC_ADMIN" => Ok(MAC_ADMIN),
            "MAC_OVERRIDE" => Ok(MAC_OVERRIDE),
            "MKNOD" => Ok(MKNOD),
            "NET_ADMIN" => Ok(NET_ADMIN),
            "NET_BIND_SERVICE" => Ok(NET_BIND_SERVICE),
            "NET_BROADCAST" => Ok(NET_BROADCAST),
            "NET_RAW" => Ok(NET_RAW),
            "SETFCAP" => Ok(SETFCAP),
            "SETGID" => Ok(SETGID),
            "SETPCAP" => Ok(SETPCAP),
            "SETUID" => Ok(SETUID),
            "SYS_ADMIN" => Ok(SYS_ADMIN),
            "SYS_BOOT" => Ok(SYS_BOOT),
            "SYS_CHROOT" => Ok(SYS_CHROOT),
            "SYS_MODULE" => Ok(SYS_MODULE),
            "SYS_NICE" => Ok(SYS_NICE),
            "SYS_PACCT" => Ok(SYS_PACCT),
            "SYS_PTRACE" => Ok(SYS_PTRACE),
            "SYS_RAWIO" => Ok(SYS_RAWIO),
            "SYS_RESOURCE" => Ok(SYS_RESOURCE),
            "SYS_TIME" => Ok(SYS_TIME),
            "SYS_TTY_CONFIG" => Ok(SYS_TTY_CONFIG),
            "SYSLOG" => Ok(SYSLOG),
            "WAKE_ALARM" => Ok(WAKE_ALARM),
            _ => Err(s.to_string()),
        }
    }
}

deserialize_from_str!(Capability);

macro_rules! set (
    { $($value:expr),+ } => {
        {
            let mut m = Set::new();
            $(
                m.insert($value);
            )+
            m
        }
     };
);

#[derive(Clone, Debug, Default)]
pub struct CapSet(Set<Capability>);

impl CapSet {
    // fn default_caps() -> CapSet {
    //     use Capability::*;
    //     CapSet(set![
    //         AUDIT_WRITE,
    //         CHOWN,
    //         DAC_OVERRIDE,
    //         FOWNER,
    //         FSETID,
    //         KILL,
    //         MKNOD,
    //         NET_BIND_SERVICE,
    //         NET_RAW,
    //         SETFCAP,
    //         SETGID,
    //         SETPCAP,
    //         SETUID,
    //         SYS_CHROOT
    //     ])
    // }
    fn empty_caps() -> CapSet {
        CapSet(Set::new())
    }
    fn all_caps() -> CapSet {
        use Capability::*;
        CapSet(set![
            AUDIT_CONTROL,
            AUDIT_READ,
            AUDIT_WRITE,
            BLOCK_SUSPEND,
            CHOWN,
            DAC_OVERRIDE,
            DAC_READ_SEARCH,
            FOWNER,
            FSETID,
            IPC_LOCK,
            IPC_OWNER,
            KILL,
            LEASE,
            LINUX_IMMUTABLE,
            MAC_ADMIN,
            MAC_OVERRIDE,
            MKNOD,
            NET_ADMIN,
            NET_BIND_SERVICE,
            NET_BROADCAST,
            NET_RAW,
            SETFCAP,
            SETGID,
            SETPCAP,
            SETUID,
            SYS_ADMIN,
            SYS_BOOT,
            SYS_CHROOT,
            SYS_MODULE,
            SYS_NICE,
            SYS_PACCT,
            SYS_PTRACE,
            SYS_RAWIO,
            SYS_RESOURCE,
            SYS_TIME,
            SYS_TTY_CONFIG,
            SYSLOG,
            WAKE_ALARM
        ])
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for CapSet {
    fn deserialize<D>(deserializer: D) -> Result<CapSet, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CapSetVisitor;

        impl<'de> Visitor<'de> for CapSetVisitor {
            type Value = CapSet;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("capability list")
            }

            fn visit_unit<E>(self) -> Result<CapSet, E> {
                Ok(CapSet::empty_caps())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<CapSet, A::Error>
            where
                A: SeqAccess<'de>,
                A::Error: de::Error,
            {
                let mut s = Set::new();
                while let Some(value) = seq.next_element::<String>()? {
                    match Capability::from_str(&value) {
                        Ok(cap) => {
                            s.insert(cap);
                        }
                        _ => {
                            if value.to_uppercase() == "ALL" {
                                return Ok(CapSet::all_caps());
                            } else {
                                return Err(de::Error::custom("not a capability"));
                            }
                        }
                    }
                }
                Ok(CapSet(s))
            }
        }

        deserializer.deserialize_any(CapSetVisitor)
    }
}

lazy_static! {
    static ref ALL_CAPS: Set<Capability> = CapSet::all_caps().0;
    static ref NUMBER_OF_CAPS: usize = ALL_CAPS.len();
}

impl Serialize for CapSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let n = self.0.len();

        if n == *NUMBER_OF_CAPS {
            let mut seq = serializer.serialize_seq(Some(1))?;
            seq.serialize_element("ALL")?;
            seq.end()
        } else {
            let mut seq = serializer.serialize_seq(Some(n))?;
            for e in self.0.iter() {
                seq.serialize_element(&e)?;
            }
            seq.end()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Capabilities {
    #[serde(default)]
    #[serde(skip_serializing_if = "CapSet::is_empty")]
    cap_add: CapSet,
    #[serde(default)]
    #[serde(skip_serializing_if = "CapSet::is_empty")]
    cap_drop: CapSet,
}

/*
impl Capabilities {
    pub fn empty() -> Capabilities {
        Capabilities {
            cap_add: CapSet::new(),
            cap_drop: CapSet::default_caps(),
        }
    }
    pub fn all() -> Capabilities {
        Capabilities {
            cap_add: CapSet::other_caps(),
            cap_drop: CapSet::new(),
        }
    }
    pub fn capabilities(&self) -> CapSet {
        let mut set = CapSet::default_caps().0;
        for e in self.cap_add.0.iter() {
            set.insert(e.clone());
        }
        for e in self.cap_drop.0.iter() {
            set.remove(e);
        }
        CapSet(set)
    }
    pub fn add(&mut self, c: Capability) {
        self.cap_drop.0.remove(&c);
        if !DEFAULT_CAPS.contains(&c) {
            self.cap_add.0.insert(c);
        }
    }
    pub fn drop(&mut self, c: Capability) {
        self.cap_add.0.remove(&c);
        if !DEFAULT_CAPS.contains(&c) {
            self.cap_drop.0.insert(c);
        }
    }
}
*/
