use strict;
use warnings;

local $/;
my $qualification = <STDIN>;
my @architectures = $qualification =~ /"architecture":"([^"]*)"/g;

if (@architectures != 1
    || ($architectures[0] ne 'x86_64' && $architectures[0] ne 'aarch64')) {
    print STDERR "model qualification requires exactly one supported runtime architecture\n";
    exit 1;
}

$qualification =~ s/"architecture":"[^"]*"/"architecture":"<architecture>"/;
$qualification =~ s/"cpu":"[^"]*"/"cpu":"<cpu>"/;
$qualification =~ s/"rustc":"[^"]*"/"rustc":"<rustc>"/;

print $qualification;
