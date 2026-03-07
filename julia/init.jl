import BeforeIT as Bit
using JSON3

const MODELS = Dict{Int, Any}()
const _NEXT_ID = Ref(1)

function egopol_init_model(params_json::String)::Int
    params = JSON3.read(params_json)

    parameters = deepcopy(Bit.AUSTRIA2010Q1.parameters)
    initial_conditions = deepcopy(Bit.AUSTRIA2010Q1.initial_conditions)

    # Apply tax rate overrides if provided
    if haskey(params, :tax_rates)
        tr = params[:tax_rates]
        haskey(tr, :income)            && (parameters["tau_INC"] = tr[:income])
        haskey(tr, :corporate)         && (parameters["tau_FIRM"] = tr[:corporate])
        haskey(tr, :vat)               && (parameters["tau_VAT"] = tr[:vat])
        haskey(tr, :social_employer)   && (parameters["tau_SIF"] = tr[:social_employer])
        haskey(tr, :social_employee)   && (parameters["tau_SIW"] = tr[:social_employee])
        haskey(tr, :export)            && (parameters["tau_EXPORT"] = tr[:export])
        haskey(tr, :capital_formation) && (parameters["tau_CF"] = tr[:capital_formation])
    end

    model = Bit.Model(parameters, initial_conditions)
    Bit.collect_data!(model)

    id = _NEXT_ID[]
    _NEXT_ID[] += 1
    MODELS[id] = model

    return id
end

_safe(x) = isnan(x) || isinf(x) ? 0.0 : x

function _safe_growth(series)
    n = length(series)
    n < 2 && return 0.0
    # Find previous non-NaN value
    prev = NaN
    for i in (n - 1):-1:1
        if !isnan(series[i]) && series[i] != 0.0
            prev = series[i]
            break
        end
    end
    isnan(prev) && return 0.0
    return series[end] / prev - 1.0
end

function egopol_step!(model_id::Int)::String
    model = MODELS[model_id]

    Bit.step!(model; parallel = false)
    Bit.collect_data!(model)

    data = model.data
    t = length(data.real_gdp)
    result = Dict{Symbol, Any}(
        :quarter => t,
        :real_gdp => _safe(data.real_gdp[end]),
        :nominal_gdp => _safe(data.nominal_gdp[end]),
        :real_gdp_growth => _safe(_safe_growth(data.real_gdp)),
        :nominal_gdp_growth => _safe(_safe_growth(data.nominal_gdp)),
        :inflation => _safe(model.agg.pi_[end]),
        :unemployment => _safe(count(x -> x == 0, model.w_act.O_h) / length(model.w_act.O_h)),
        :euribor => _safe(data.euribor[end]),
        :government_spending => _safe(data.real_government_consumption[end]),
        :government_revenue => _safe(model.gov.Y_G),
        :government_debt => _safe(model.gov.L_G),
        :consumption => _safe(data.real_household_consumption[end]),
        :investment => _safe(data.real_capitalformation[end]),
        :exports => _safe(data.real_exports[end]),
        :imports => _safe(data.real_imports[end]),
        :wage_growth => _safe(_safe_growth(data.wages)),
        :price_level => _safe(model.agg.P_bar),
        :money_supply => _safe(sum(model.w_act.D_h) + sum(model.w_inact.D_h) + sum(model.firms.D_h) + model.bank.D_h),
        :bank_deposits => _safe(model.bank.D_k),
        :bank_loans => _safe(sum(model.firms.L_i)),
        :equity_index => _safe(sum(model.firms.E_i)),
        :housing_price => _safe(model.agg.P_bar_CF),
    )

    return JSON3.write(result)
end

function egopol_set_tax_rates!(model_id::Int, tax_json::String)::Nothing
    model = MODELS[model_id]
    prop = model.prop

    tr = JSON3.read(tax_json)
    haskey(tr, :income)            && (prop.tau_INC = tr[:income])
    haskey(tr, :corporate)         && (prop.tau_FIRM = tr[:corporate])
    haskey(tr, :vat)               && (prop.tau_VAT = tr[:vat])
    haskey(tr, :social_employer)   && (prop.tau_SIF = tr[:social_employer])
    haskey(tr, :social_employee)   && (prop.tau_SIW = tr[:social_employee])
    haskey(tr, :export)            && (prop.tau_EXPORT = tr[:export])
    haskey(tr, :capital_formation) && (prop.tau_CF = tr[:capital_formation])

    return nothing
end

function egopol_drop_model!(model_id::Int)::Nothing
    delete!(MODELS, model_id)
    return nothing
end
