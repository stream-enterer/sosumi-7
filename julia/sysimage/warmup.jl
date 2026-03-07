import BeforeIT as Bit
using JSON3

# Warm up all hot paths for sysimage precompilation
parameters = Bit.AUSTRIA2010Q1.parameters
initial_conditions = Bit.AUSTRIA2010Q1.initial_conditions
model = Bit.Model(parameters, initial_conditions)
Bit.collect_data!(model)

for _ in 1:4
    Bit.step!(model; parallel = false)
    Bit.collect_data!(model)
end

# Exercise JSON3 serialization
result = Dict(:gdp => model.data.real_gdp[end], :euribor => model.data.euribor[end])
json_str = JSON3.write(result)
JSON3.read(json_str)
