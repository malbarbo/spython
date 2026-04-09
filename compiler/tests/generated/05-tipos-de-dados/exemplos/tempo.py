from dataclasses import dataclass

@dataclass
class Tempo:
    '''
    Representa o tempo de duração de um evento. horas, minutos e segundos devem
    ser positivos. minutos e segundos devem ser menores que 60.
    '''
    horas: int
    minutos: int
    segundos: int

def segundos_para_tempo(segundos: int) -> Tempo:
    '''
    Converte a quantidade *segundos* para o tempo equivalente em horas, minutos
    e segundos. A quantidade de segundos e minutos da resposta é sempre menor
    que 60.

    Requer que segundos seja não negativo.

    Exemplos
    >>> # 160 // 60 -> 2 mins, 160 % 60 -> 40 segs
    >>> segundos_para_tempo(160)
    Tempo(horas=0, minutos=2, segundos=40)
    >>> # 3760 // 3600 -> 1 hora
    >>> # 3760 % 3600 -> 160 segundos que sobraram
    >>> # 160 // 60 -> 2 mins, 160 % 60 -> 40 segs
    >>> segundos_para_tempo(3760)
    Tempo(horas=1, minutos=2, segundos=40)
    '''
    h = segundos // 3600
    # segundos que não foram convertidos para hora
    restantes = segundos % 3600
    m = restantes // 60
    s = restantes % 60
    return Tempo(h, m, s)

def tempo_para_string(t: Tempo) -> str:
    '''
    Converte *t* em uma mensagem para o usuário. Cada componente de *t* aparece
    com a sua unidade, mas se o valor do componente for 0, ele não aparece na
    mensagem. Os componentes são separados com "e" ou "," respeitando as regras
    do Português. Se *t* for Tempo(0, 0, 0), devolve "0 segundo(s)".

    Exemplos
    >>> # horas == 0 and minutos == 0
    >>> tempo_para_string(Tempo(0, 0, 0))
    '0 segundo(s)'
    >>> tempo_para_string(Tempo(0, 0, 1))
    '1 segundo(s)'
    >>> tempo_para_string(Tempo(0, 0, 10))
    '10 segundo(s)'

    >>> # horas == 0 and minutos != 0 \
    >>> #            and segundos != 0
    >>> tempo_para_string(Tempo(0, 1, 20))
    '1 minuto(s) e 20 segundo(s)'

    >>> # horas == 0 and minutos != 0 \
    >>> #            and segundos == 0
    >>> tempo_para_string(Tempo(0, 2, 0))
    '2 minuto(s)'

    >>> # horas != 0 and minutos != 0 and segundos != 0
    >>> tempo_para_string(Tempo(1, 2, 1))
    '1 hora(s), 2 minuto(s) e 1 segundo(s)'

    >>> # horas != 0 and minutos == 0 and segundos != 0
    >>> tempo_para_string(Tempo(4, 0, 25))
    '4 hora(s) e 25 segundo(s)'

    >>> # horas != 0 and minutos != 0 and segundos == 0
    >>> tempo_para_string(Tempo(2, 4, 0))
    '2 hora(s) e 4 minuto(s)'

    >>> # horas != 0 and minutos == 0 and segundos == 0
    >>> tempo_para_string(Tempo(3, 0, 0))
    '3 hora(s)'
    '''
    h = str(t.horas) + ' hora(s)'
    m = str(t.minutos) + ' minuto(s)'
    s = str(t.segundos) + ' segundo(s)'
    # Temos 7 formas distintas
    if t.horas > 0:
        if t.minutos > 0:
            if t.segundos > 0:
                msg = h + ', ' + m + ' e ' + s
            else:
                msg = h + ' e ' + m
        elif t.segundos > 0:
            msg = h + ' e ' + s
        else:
            msg = h
    elif t.minutos > 0:
        if t.segundos > 0:
            msg = m + ' e ' + s
        else:
            msg = m
    else:
        msg = s
    return msg

# Generated from doctests.
assert segundos_para_tempo(160) == Tempo(horas=0, minutos=2, segundos=40)
assert segundos_para_tempo(3760) == Tempo(horas=1, minutos=2, segundos=40)
assert tempo_para_string(Tempo(0, 0, 0)) == '0 segundo(s)'
assert tempo_para_string(Tempo(0, 0, 1)) == '1 segundo(s)'
assert tempo_para_string(Tempo(0, 0, 10)) == '10 segundo(s)'
assert tempo_para_string(Tempo(0, 1, 20)) == '1 minuto(s) e 20 segundo(s)'
assert tempo_para_string(Tempo(0, 2, 0)) == '2 minuto(s)'
assert tempo_para_string(Tempo(1, 2, 1)) == '1 hora(s), 2 minuto(s) e 1 segundo(s)'
assert tempo_para_string(Tempo(4, 0, 25)) == '4 hora(s) e 25 segundo(s)'
assert tempo_para_string(Tempo(2, 4, 0)) == '2 hora(s) e 4 minuto(s)'
assert tempo_para_string(Tempo(3, 0, 0)) == '3 hora(s)'
