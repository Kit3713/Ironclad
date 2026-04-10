# Full system declaration demonstrating all Ironclad domains
# This file exercises the complete Phase 1 grammar

import "base/hardened.ic"

var env = production
var root_size = 50G

# Base class with security defaults
class hardened_base {
    selinux {
        mode = enforcing
        type = targeted

        user system_u {
            roles = [system_r]
            range = s0-s15:c0.c1023
        }

        booleans {
            httpd_can_network_connect = true
        }
    }

    firewall {
        table inet filter {
            chain input {
                type = filter
                hook = input
                priority = 0
                policy = drop

                rule allow_established {
                    match {
                        ct_state = established
                    }
                    action = accept
                }

                rule allow_icmp {
                    match {
                        protocol = icmp
                    }
                    action = accept
                }
            }

            chain output {
                type = filter
                hook = output
                priority = 0
                policy = accept
            }
        }
    }

    users {
        policy {
            complexity {
                min_length = 12
                require_uppercase = true
                require_digit = true
                require_special = true
            }
            lockout {
                attempts = 5
                lockout_time = 900
            }
        }
    }
}

# Web server system extending the hardened base
system web_server extends hardened_base {
    # Storage layout
    disk /dev/sda {
        label = gpt

        fat32 efi {
            index = 1
            size = 1G
            type = ef00
            mount = /boot/efi [nodev, nosuid, noexec]
        }

        ext4 boot {
            index = 2
            size = 1G
            mount = /boot [nodev, nosuid, noexec]
        }
    }

    disk /dev/nvme0n1 {
        label = gpt

        luks2 system {
            index = 1
            size = remaining
            cipher = aes-xts-plain64
            key_size = 512

            lvm vg_system {
                ext4 root {
                    size = 50G
                    mount = /
                }

                ext4 var {
                    size = 100G
                    mount = /var [nodev, nosuid, noexec]
                }

                swap swap0 { size = 16G }
            }
        }
    }

    # Network configuration
    network {
        backend = networkmanager

        interface eth0 {
            type = ethernet
            ip {
                address = "10.0.0.10/24"
                gateway = "10.0.0.1"
            }
        }

        dns {
            servers = ["10.0.0.1", "8.8.8.8"]
            search = ["example.com"]
        }
    }

    # Additional firewall rules for web traffic
    firewall {
        table inet filter {
            chain input {
                rule allow_ssh {
                    match {
                        protocol = tcp
                        dport = 22
                        iif = eth0
                    }
                    action = accept
                    log {
                        prefix = "SSH: "
                        level = info
                    }
                }

                rule allow_http {
                    match {
                        protocol = tcp
                        dport = 80
                    }
                    action = accept
                }

                rule allow_https {
                    match {
                        protocol = tcp
                        dport = 443
                    }
                    action = accept
                }
            }
        }
    }

    # Packages
    packages {
        repo baseos {
            name = "BaseOS"
            baseurl = "https://mirror.example.com/baseos"
            gpgcheck = true
        }

        repo appstream {
            name = "AppStream"
            baseurl = "https://mirror.example.com/appstream"
            gpgcheck = true
        }

        pkg httpd { state = present }
        pkg mod_ssl { state = present }
        pkg php { state = present }
        pkg chrony { state = present }
        pkg rsyslog { state = present }
        pkg telnet { state = absent }

        group "Security Tools" { state = present }
    }

    # User accounts
    users {
        user webadmin {
            uid = 1001
            groups = [wheel, webops]
            shell = /bin/bash
            home = /home/webadmin
            selinux_user = staff_u
        }

        user deployer {
            uid = 1002
            groups = [webops]
            shell = /bin/bash
            home = /home/deployer
            system = false
        }

        group webops {
            gid = 2001
        }
    }

    # Services
    init systemd {
        defaults {
            restart = on-failure
            restart_sec = 5
        }

        journal {
            storage = persistent
            max_use = 500M
            compress = true
        }

        service httpd {
            type = notify
            exec_start = /usr/sbin/httpd
            enabled = true

            hardening {
                protect_system = strict
                protect_home = true
                no_new_privileges = true
                private_tmp = true
            }

            logging {
                stdout = journal
                stderr = journal
            }
        }

        service chronyd {
            type = forking
            exec_start = /usr/sbin/chronyd
            enabled = true
        }

        service rsyslog {
            type = notify
            exec_start = /usr/sbin/rsyslogd
            enabled = true
        }
    }
}
