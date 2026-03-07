using PackageCompiler

sysimage_name = if Sys.iswindows()
    "egopol_sysimage.dll"
elseif Sys.isapple()
    "egopol_sysimage.dylib"
else
    "egopol_sysimage.so"
end

output_path = joinpath(@__DIR__, sysimage_name)
warmup_path = joinpath(@__DIR__, "warmup.jl")

println("Building sysimage at $output_path ...")

create_sysimage(
    [:BeforeIT, :JSON3];
    sysimage_path = output_path,
    precompile_execution_file = warmup_path,
)

println("Done. Sysimage written to $output_path")
