# %%
import loki

# %%
df = loki.get_df(interactive="btrJOA")

# %%
df *= -1
# %%
loki.output(df)
# %%
